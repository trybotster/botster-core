//! Input processing of the fake worker: refusals, paste, file transfer and the PTY write path.

use crate::*;

pub(crate) struct Paste {
    op: Op,
    buf: Vec<u8>,
    require_bracketed: bool,
}

pub(crate) struct FileUpload {
    op: Op,
    name: String,
    buf: Vec<u8>,
}

impl FakeWorker {
    /// The client writes bytes to its end of the route (Codec TS-3). On a chunked route one write is one DataChannel message.
    pub fn write(&mut self, id: RouteId, bytes: &[u8]) {
        let framing = self.cfg.framing;
        let Some(route) = self.routes.get_mut(&id) else {
            return;
        };
        if route.eof.is_some() {
            return;
        }
        match framing {
            Framing::Stream => route.reader.push(bytes),
            Framing::Chunked => route.inbound.push_back(bytes.to_vec()),
        }
        self.process_inbound(id);
    }

    /// The bytes that the worker has not read from the route (Codec TS-5, TS-7).
    pub fn unread(&self, id: RouteId) -> usize {
        self.routes.get(&id).map_or(0, |r| {
            r.reader.pending() + r.inbound.iter().map(Vec::len).sum::<usize>()
        })
    }

    /// True when the worker has input waiting on the route and does not read it (Codec TS-5, TS-7).
    pub fn input_blocked(&self, id: RouteId) -> bool {
        self.unread(id) > 0 && !self.can_read_input(id)
    }

    /// The exception frames that PTY writes can still add: each waiting write may end in a refusal or a completion (Codec TS-7).
    fn reserved_exceptions(&self) -> u64 {
        self.pty_pending.iter().filter(|w| !w.silent).count() as u64
    }

    fn can_read_input(&self, id: RouteId) -> bool {
        let Some(r) = self.routes.get(&id) else {
            return false;
        };
        // Codec TS-7: the worker stops reading input while the outgoing queue cannot hold one more exception frame, counting those that
        // waiting writes may add. Codec TS-5: it stops reading while its input queue is full, so input is never dropped.
        r.eof.is_none()
            && !r.closing
            && r.out_bytes + (self.reserved_exceptions() + 1) * self.worst_exception()
                <= self.cfg.route_queue_bytes
            && (!self.pty_blocked || self.pty_queued_bytes() < self.cfg.input_queue_bytes as usize)
    }

    pub(crate) fn process_inbound(&mut self, id: RouteId) {
        while self.can_read_input(id) {
            let bounds = self.bounds();
            let route = self.routes.get_mut(&id).expect("route");
            let next = match self.cfg.framing {
                Framing::Stream => route.reader.next_frame(&bounds),
                Framing::Chunked => match route.inbound.pop_front() {
                    Some(message) => route.chunks.push_message(&message, &bounds),
                    None => return,
                },
            };
            match next {
                Ok(Some(frame)) => self.handle_frame(id, &frame),
                Ok(None) if self.cfg.framing == Framing::Stream => return,
                Ok(None) => {}
                Err(e) => return self.protocol_error(id, e),
            }
        }
    }

    /// Codec TS-3: on a route, a protocol error ends it with `route_closed { protocol_error { code } }`.
    fn protocol_error(&mut self, id: RouteId, e: DecodeError) {
        self.close_route(id, CloseReason::ProtocolError { code: e.code }, None);
    }

    fn handle_frame(&mut self, id: RouteId, frame: &[u8]) {
        match ToWorker::decode(frame, &self.bounds()) {
            Ok(Decoded::Frame(f)) => self.handle(id, f),
            // Codec TS-3: an unknown ignorable frame is dropped.
            Ok(Decoded::Ignored { .. }) => {}
            // Codec TS-5b: an unknown value in a parseable input is refused, and the route stays open.
            Ok(Decoded::Unsupported { op, what }) => {
                self.refuse(id, op, RefusalReason::Unsupported { what })
            }
            // Codec TS-5b (Core IN-9): an input that breaks a field constraint has no effect and is refused.
            Ok(Decoded::Invalid { op, .. }) => self.refuse(id, op, RefusalReason::Refused),
            Err(e) => self.protocol_error(id, e),
        }
    }

    pub(crate) fn refuse(&mut self, id: RouteId, op: Option<Op>, reason: RefusalReason) {
        self.refuse_with(id, op, reason, None, None);
    }

    fn refuse_with(
        &mut self,
        id: RouteId,
        op: Option<Op>,
        reason: RefusalReason,
        written: Option<u64>,
        path: Option<String>,
    ) {
        let op = op.filter(|o| o.0 != 0);
        self.exception(
            id,
            ToClient::InputRefused(InputRefused {
                op,
                reason,
                written_bytes: written,
                path,
            }),
        );
    }

    /// The checks that every input passes first. Returns false when the input was refused.
    fn admitted(&mut self, id: RouteId, op: Option<Op>, allowed_when_view_only: bool) -> bool {
        let route = &self.routes[&id];
        let (live, writable) = (route.ever_live, route.writable);
        if self.exited.is_some() {
            // Codec TS-5b: session_ended.
            self.refuse(id, op, RefusalReason::SessionEnded);
            false
        } else if !live {
            // Codec TS-4b: input before live is refused not_ready.
            self.refuse(id, op, RefusalReason::NotReady);
            false
        } else if !writable && !allowed_when_view_only {
            // Codec TS-1, TS-5b: a view-only route refuses every input except a query reply and a resize.
            self.refuse(id, op, RefusalReason::NotWritable);
            false
        } else {
            true
        }
    }

    fn handle(&mut self, id: RouteId, frame: ToWorker) {
        match frame {
            ToWorker::Bytes { op, bytes } => {
                self.touch(id);
                // Codec BP-3: raw bytes reach the PTY verbatim.
                if self.admitted(id, Some(op), false) {
                    self.write_pty(id, Some(op), Action::Raw(bytes.to_vec()), false);
                }
            }
            ToWorker::Text { op, text } => {
                self.touch(id);
                // Codec BP-3: text is written as one transaction.
                if self.admitted(id, Some(op), false) {
                    self.write_pty(id, Some(op), Action::Raw(text.into_bytes()), false);
                }
            }
            ToWorker::PasteChunk { op, bytes } => {
                self.touch(id);
                self.paste_chunk(id, op, bytes.to_vec());
            }
            ToWorker::FileChunk { op, bytes } => self.file_chunk(id, op, bytes.to_vec()),
            ToWorker::Input(input) => self.input(id, input),
        }
    }

    fn touch(&mut self, id: RouteId) {
        // Core IN-4, IN-10: the client revision advances when a complete client input frame is received, even if it is refused later.
        self.client_rev += 1;
        self.activity_routes.push(id);
        let at = self.next_activity();
        self.routes.get_mut(&id).expect("route").last_activity = at;
        // Codec TS-6: the route with the most recent activity may be a different one now.
        self.apply_size_policy();
    }

    fn input(&mut self, id: RouteId, input: Input) {
        let op = input.op();
        if !matches!(
            input,
            Input::ReplyTerminalQuery { .. }
                | Input::FileBegin { .. }
                | Input::FileCommit { .. }
                | Input::FileAbort { .. }
        ) {
            // Codec TS-6: any input is activity. A query reply is not (Codec TS-7). A file frame is not terminal input (Codec TS-5f).
            self.touch(id);
        }
        let view_only_ok = matches!(
            input,
            Input::ReplyTerminalQuery { .. } | Input::Resize { .. }
        );
        if !self.admitted(id, op, view_only_ok) {
            return;
        }
        // Core IN-9: the worst-case size of a semantic input is checked before the input is admitted. The script supplies the number.
        if self.admission_size(&input) > self.routes[&id].limits.max_paste_bytes {
            return self.refuse(id, op, RefusalReason::TooLarge);
        }
        match input {
            Input::Key { .. } | Input::Mouse { .. } => {
                self.write_pty(id, op, Action::Semantic(input), false)
            }
            Input::Focus { focused, .. } => {
                // Codec TS-10: the route's latest focus input is remembered.
                self.routes.get_mut(&id).expect("route").last_focus = focused;
                self.write_pty(id, op, Action::Semantic(input), false);
            }
            Input::Resize { rows, cols, .. } => {
                // Codec TS-6: the route reports its size, and the policy picks the effective one. A resize has no answer.
                self.routes.get_mut(&id).expect("route").last_size = Some((rows, cols));
                self.apply_size_policy();
            }
            Input::ReplyTerminalQuery {
                query_id, reply, ..
            } => self.reply(id, op, &query_id, reply),
            Input::PasteBegin {
                op,
                total_bytes,
                require_bracketed,
            } => {
                let max = self.routes[&id].limits.max_paste_bytes;
                if self.routes[&id].paste.is_some() {
                    // Codec TS-5d: a second paste_begin while a paste is open.
                    self.refuse(id, Some(op), RefusalReason::Refused);
                } else if total_bytes.is_some_and(|t| t > max) {
                    // Codec TS-5d: total_bytes is advisory; the worker may refuse at paste_begin.
                    self.refuse(id, Some(op), RefusalReason::TooLarge);
                } else {
                    self.routes.get_mut(&id).expect("route").paste = Some(Paste {
                        op,
                        buf: vec![],
                        require_bracketed: require_bracketed.unwrap_or(false),
                    });
                }
            }
            Input::PasteCommit { op, want_result } => {
                self.paste_commit(id, op, want_result.unwrap_or(false))
            }
            Input::PasteAbort { op } => match &self.routes[&id].paste {
                // Codec TS-5d: an abort before the commit writes nothing, with no frame.
                Some(p) if p.op == op => self.routes.get_mut(&id).expect("route").paste = None,
                _ => self.refuse(id, Some(op), RefusalReason::Refused),
            },
            Input::FileBegin {
                op,
                name,
                total_bytes,
                ..
            } => {
                let max = self.routes[&id].limits.max_file_bytes;
                if self.routes[&id].file.is_some() {
                    // Codec TS-5f: one file is open per route.
                    self.refuse(id, Some(op), RefusalReason::Refused);
                } else if total_bytes.is_some_and(|t| t > max) {
                    // Codec TS-5f: the worker may refuse at file_begin; nothing is created.
                    self.refuse(id, Some(op), RefusalReason::TooLarge);
                } else {
                    self.routes.get_mut(&id).expect("route").file = Some(FileUpload {
                        op,
                        name,
                        buf: vec![],
                    });
                }
            }
            Input::FileCommit { op } => self.file_commit(id, op),
            Input::FileAbort { op } => match &self.routes[&id].file {
                // Codec TS-5f: an abort before the commit writes nothing and removes partial data, with no frame.
                Some(f) if f.op == op => self.routes.get_mut(&id).expect("route").file = None,
                _ => self.refuse(id, Some(op), RefusalReason::Refused),
            },
        }
    }

    fn paste_chunk(&mut self, id: RouteId, op: Op, bytes: Vec<u8>) {
        if !self.admitted(id, Some(op), false) {
            return;
        }
        let max = self.routes[&id].limits.max_paste_bytes as usize;
        let route = self.routes.get_mut(&id).expect("route");
        match &mut route.paste {
            Some(p) if p.op == op => {
                p.buf.extend(bytes);
                if p.buf.len() > max {
                    // Codec TS-5d: too_large at the first chunk that exceeds the bound, with zero effect.
                    route.paste = None;
                    self.refuse(id, Some(op), RefusalReason::TooLarge);
                }
            }
            // Codec TS-5d: a chunk with no open paste, or with another op.
            _ => self.refuse(id, Some(op), RefusalReason::Refused),
        }
    }

    fn paste_commit(&mut self, id: RouteId, op: Op, want_result: bool) {
        let route = self.routes.get_mut(&id).expect("route");
        let Some(p) = route.paste.take_if(|p| p.op == op) else {
            // Codec TS-5d: a commit with no open paste, or with another op.
            return self.refuse(id, Some(op), RefusalReason::Refused);
        };
        // Codec TS-5e: the mode is checked when the write starts.
        let action = Action::Paste {
            require_bracketed: p.require_bracketed,
            want_result,
        };
        self.write_pty(id, Some(op), action, false);
    }

    fn file_chunk(&mut self, id: RouteId, op: Op, bytes: Vec<u8>) {
        if !self.admitted(id, Some(op), false) {
            return;
        }
        let max = self.routes[&id].limits.max_file_bytes as usize;
        let route = self.routes.get_mut(&id).expect("route");
        match &mut route.file {
            Some(f) if f.op == op => {
                f.buf.extend(bytes);
                if f.buf.len() > max {
                    // Codec TS-5f: over the bound at the first chunk that exceeds it, and nothing of it stays.
                    route.file = None;
                    self.refuse(id, Some(op), RefusalReason::TooLarge);
                }
            }
            _ => self.refuse(id, Some(op), RefusalReason::Refused),
        }
    }

    fn file_commit(&mut self, id: RouteId, op: Op) {
        let route = self.routes.get_mut(&id).expect("route");
        let Some(file) = route.file.take_if(|f| f.op == op) else {
            return self.refuse(id, Some(op), RefusalReason::Refused);
        };
        // Codec TS-5f: the file is written, and its path pasted, at the commit's position in the receive order.
        let action = Action::File {
            name: file.name,
            content: file.buf,
        };
        self.write_pty(id, Some(op), action, false);
    }

    // ---- terminal query replies (Codec TS-7, TS-5b) ----

    fn reply(&mut self, id: RouteId, op: Option<Op>, qid: &str, reply: QueryReply) {
        let Some(q) = self.queries.get(qid) else {
            return self.refuse(id, op, RefusalReason::UnknownQuery);
        };
        // Codec TS-7 (binding): a reply from a route the query was not bound to is wrong_route.
        if q.route != id {
            return self.refuse(id, op, RefusalReason::WrongRoute);
        }
        match q.state {
            QueryState::FallbackDone | QueryState::Replied => {
                return self.refuse(id, op, RefusalReason::AlreadyReplied)
            }
            // Codec TS-7: while the fallback is still pending, a client answer is query_expired.
            QueryState::FallbackWaiting => return self.refuse(id, op, RefusalReason::QueryExpired),
            QueryState::Asked => {}
        }
        match reply {
            // Codec TS-7: a decline is not a final answer; it triggers the fallback.
            QueryReply::Decline => self.fallback(qid),
            QueryReply::Encoded { bytes_base64 } => {
                let Some(bytes) = base64_decode(&bytes_base64) else {
                    return self.refuse(id, op, RefusalReason::InvalidInput);
                };
                if bytes.len() as u64 > self.routes[&id].limits.max_query_reply_bytes {
                    return self.refuse(id, op, RefusalReason::TooLarge);
                }
                self.queries.get_mut(qid).expect("query").state = QueryState::Replied;
                self.write_pty(id, op, Action::Raw(bytes), false);
            }
            // Codec TS-7: a typed variant is written by the worker. The script supplies the bytes.
            _ => {
                self.queries.get_mut(qid).expect("query").state = QueryState::Replied;
                let bytes = self.cfg.script.writes.reply.clone();
                self.write_pty(id, op, Action::Raw(bytes), false);
            }
        }
    }

    // ---- the PTY (Codec TS-5, TS-5b) ----

    /// Writes to the PTY in receive order. While the PTY blocks, writes wait in the input queue.
    pub(crate) fn write_pty(
        &mut self,
        route: RouteId,
        op: Option<Op>,
        action: Action,
        silent: bool,
    ) {
        let w = PendingWrite {
            route,
            op,
            action,
            silent,
        };
        // Core AM-2: a host write in progress is contiguous, so route input waits behind it.
        if self.pty_blocked || self.host_txn.is_some() || !self.pty_pending.is_empty() {
            self.pty_pending.push_back(w);
        } else {
            self.do_write(w);
        }
    }

    /// One write to the PTY. Guards are checked, and the scripted bytes are chosen, with the scripted modes now (Codec TS-5e, Core IN-9).
    pub(crate) fn do_write(&mut self, w: PendingWrite) {
        let (id, op) = (w.route, w.op);
        let modes = self.modes.clone();
        let mut done_path = None;
        let mut want_done = false;
        let bytes = match w.action {
            Action::Host { .. } => return,
            Action::Raw(b) => b,
            Action::Semantic(input) => {
                // Codec TS-5b: the script supplies the whole outcome. Bytes are written, a not-reported input writes nothing and
                // sends no frame, and an unsupported one is refused. The fake never reads the modes here.
                let writes = &self.cfg.script.writes;
                let outcome = match input {
                    Input::Key { .. } => &writes.key,
                    Input::Mouse { .. } => &writes.mouse,
                    Input::Focus { focused: true, .. } => &writes.focus_in,
                    _ => &writes.focus_out,
                };
                match outcome.clone() {
                    Outcome::Bytes(b) => b,
                    Outcome::NotReported => return,
                    Outcome::Unsupported(what) => {
                        return self.refuse(id, op, RefusalReason::Unsupported { what })
                    }
                }
            }
            Action::Paste {
                require_bracketed,
                want_result,
            } => {
                if require_bracketed && !modes.bracketed_paste {
                    // Codec TS-5e: a failed precondition has zero effect.
                    return self.refuse(id, op, RefusalReason::ModePreconditionFailed);
                }
                want_done = want_result;
                // Codec TS-5d: the script supplies the complete bytes of a paste.
                self.cfg.script.writes.paste.clone()
            }
            Action::File { name, content } => {
                self.file_counter += 1;
                let component = file_component(
                    &format!("{}-", self.file_counter),
                    &name,
                    self.cfg.file_name_limit,
                );
                let path = format!("{}/{}", self.cfg.file_directory, component);
                // Codec TS-5f: the scripted size of the path paste must fit max_paste_bytes, and the check comes before the file is
                // created. The script supplies the complete bytes of the path paste.
                let paste = self.cfg.script.writes.file.clone();
                let size = self.cfg.script.admission.file;
                let max = self
                    .routes
                    .get(&id)
                    .map_or(u64::MAX, |r| r.limits.max_paste_bytes);
                if size > max {
                    return self.refuse(id, op, RefusalReason::TooLarge);
                }
                if self.fail_file_write {
                    // Codec TS-5f: a file that cannot be written on the host is refused write_failed; no path is written.
                    return self.refuse(id, op, RefusalReason::WriteFailed);
                }
                self.files.insert(path.clone(), content);
                done_path = Some(path);
                want_done = true;
                paste
            }
        };
        let result = match self.pty_fail_after.take() {
            Some(n) => {
                // Codec TS-5b: the write to the PTY failed after n bytes.
                let n = n.min(bytes.len());
                self.log_write(op, &bytes[..n]);
                Err(n as u64)
            }
            None => {
                self.log_write(op, &bytes);
                Ok(bytes.len() as u64)
            }
        };
        match result {
            Ok(n) if want_done => {
                // Codec TS-5b, TS-5f: a paste that asked for completion, and every file commit, are answered with input_done.
                if let Some(op) = op {
                    self.exception(
                        id,
                        ToClient::InputDone(InputDone {
                            op,
                            written_bytes: n,
                            path: done_path,
                        }),
                    );
                }
            }
            Err(n) if !w.silent => {
                self.refuse_with(id, op, RefusalReason::Failed, Some(n), done_path)
            }
            _ => {}
        }
    }

    /// Records bytes that reached the PTY. A write of no bytes leaves no segment.
    pub(crate) fn log_write(&mut self, op: Option<Op>, bytes: &[u8]) {
        if !bytes.is_empty() {
            self.pty_log.push((op, bytes.to_vec()));
        }
    }

    pub(crate) fn flush_pty(&mut self) {
        while !self.pty_blocked && self.host_txn.is_none() {
            let Some(at) = self.next_source() else {
                break;
            };
            let w = self.pty_pending.remove(at).expect("index");
            self.admit(w);
        }
        let waiting: Vec<String> = self
            .queries
            .iter()
            .filter(|(_, q)| q.state == QueryState::FallbackWaiting)
            .map(|(k, _)| k.clone())
            .collect();
        for q in waiting {
            self.queries.get_mut(&q).expect("query").state = QueryState::Asked;
            self.fallback(&q);
        }
        let ids: Vec<RouteId> = self.routes.keys().copied().collect();
        for id in ids {
            self.process_inbound(id);
        }
    }
}
