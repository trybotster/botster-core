//! The channel layer that FakeHub and FakePlugin share: lanes, framing, the receive order and the counters.
//!
//! The fakes have no clock, no thread and no OS randomness (design 7.2, rule 2). Every open order is one named [`Choice`] drawn from a ChaCha8 generator.

use botster_plugin_contract::prelude::*;
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};

/// The open orders of HP section 19, as named choice points. Each cites its clause (design 7.2, rule 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Choice {
    /// Which lane's frame goes onto the wire next. Clause: HP MD-1 (no order between lanes).
    LaneOrder,
    /// Whether the sender cuts a write after this frame or batches the next one with it. Clause: HP MD-1, WF-1.
    Batching,
    /// The order in which concurrent callers' requests are admitted. Clause: HP IV-4, IV-6.
    ConcurrentInvoke,
    /// The delivery order across emitters. Clause: HP EV-4, EV-10.
    EmitterOrder,
    /// Whether a `Result` or a `Cancel` is processed first when they cross. Clause: HP IV-7.
    CancelRace,
    /// Whether a host that abandons a handler on `Cancel` still sends a late `Result`. Clause: HP IV-2, IV-5, IV-7.
    LateResult,
}

/// The seeded generator and the record of what each choice point took (for the reachability test).
pub struct Chooser {
    rng: ChaCha8Rng,
    taken: BTreeMap<Choice, Vec<usize>>,
}

impl Chooser {
    pub fn new(seed: u64) -> Self {
        Chooser {
            rng: ChaCha8Rng::seed_from_u64(seed),
            taken: BTreeMap::new(),
        }
    }

    /// One seeded decision with `n` outcomes.
    pub fn pick(&mut self, point: Choice, n: usize) -> usize {
        let pick = if n <= 1 {
            0
        } else {
            (self.rng.next_u64() % n as u64) as usize
        };
        let seen = self.taken.entry(point).or_default();
        if !seen.contains(&pick) {
            seen.push(pick);
        }
        pick
    }

    /// The raw generator, for values that are not a named order (ids and epochs start at a seeded base so a transcript must bind them).
    pub fn base(&mut self) -> u64 {
        1 + self.rng.next_u64() % 1000
    }

    pub fn taken(&self) -> &BTreeMap<Choice, Vec<usize>> {
        &self.taken
    }
}

/// A send that the peer refuses without blocking (HP MD-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backpressured;

/// What the receiver found in the next frame, in the order of steward ruling R-6.
pub enum Inbound {
    /// A known frame of the current epoch, on a legal lane.
    Frame { frame: Box<Frame>, lane: Lane },
    /// A known type that the receiver refuses as a malformed request (HP WF-2 duplicate keys, WF-3, WF-4, R-4 wrong lane). `payload` is a lenient read, for the id.
    Malformed { ty: u8, lane: u8, payload: Value },
    /// An unknown state-changing type (HP VR-2).
    Unknown { ty: u8 },
}

/// One frame the receiver was handed, for the `received` observation (HP MD-1).
#[derive(Debug, Clone, PartialEq)]
pub struct Seen {
    pub lane: &'static str,
    pub ty: &'static str,
    pub id: Value,
}

pub fn lane_name(l: Lane) -> &'static str {
    match l {
        Lane::Control => "control",
        Lane::Invoke => "invoke",
        Lane::Call => "call",
        Lane::Bulk => "bulk",
    }
}

pub fn lane_by_name(n: &str) -> Option<Lane> {
    Lane::ALL.into_iter().find(|l| lane_name(*l) == n)
}

/// One end of one connection.
pub struct Link {
    pub frame_bytes: u32,
    /// The receive bound of each lane while it is held: frames and bytes (HP MD-3, section 15).
    pub lane_caps: [(u64, u64); 4],
    /// The connection epoch. `None` before `Hello` on a host (HP section 2).
    pub epoch: Option<Epoch>,
    rx: Vec<u8>,
    /// Where the frame that is still arriving begins in `rx`; everything before it is complete.
    tail_start: usize,
    held: [bool; 4],
    parked: [Vec<Vec<u8>>; 4],
    out: [VecDeque<Vec<u8>>; 4],
    ready: VecDeque<Vec<u8>>,
    pub closed: Option<&'static str>,
    /// Frames dropped for a stale epoch (HP MD-4, CR-5).
    pub stale: u64,
    /// Unknown advisory frames ignored (HP VR-2).
    pub ignored: u64,
    pub seen: Vec<Seen>,
    /// The body bytes that were buffered for a frame whose length was refused (HP WF-1: none).
    pub refused_buffered: usize,
}

impl Link {
    pub fn new(frame_bytes: u32, lane_caps: [(u64, u64); 4]) -> Self {
        Link {
            frame_bytes,
            lane_caps,
            epoch: None,
            rx: Vec::new(),
            tail_start: 0,
            refused_buffered: 0,
            held: [false; 4],
            parked: Default::default(),
            out: Default::default(),
            ready: VecDeque::new(),
            closed: None,
            stale: 0,
            ignored: 0,
            seen: Vec::new(),
        }
    }

    /// The peer ended the connection or the epoch (HP WF-2, CR-1). Frames already queued for sending stay readable.
    pub fn close(&mut self, cause: &'static str) {
        if self.closed.is_none() {
            self.closed = Some(cause);
        }
        self.rx.clear();
        self.tail_start = 0;
    }

    /// A receiver that stops reading one lane (a slow reader). Its frames wait in a bounded queue (HP MD-3).
    pub fn hold(&mut self, lane: Lane, on: bool) {
        self.held[lane.byte() as usize] = on;
        if !on {
            for chunk in std::mem::take(&mut self.parked[lane.byte() as usize]) {
                let at = self.tail_start;
                self.tail_start += chunk.len();
                self.rx.splice(at..at, chunk);
            }
        }
    }

    /// The bytes that the frame at the tail of the buffer still needs: the rest of its header, then the rest of its body.
    fn tail_need(&self) -> usize {
        let tail = &self.rx[self.tail_start..];
        match tail.len() {
            0..=3 => 4 - tail.len(),
            n => 4 + u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]) as usize - n,
        }
    }

    /// Bytes arrive from the peer, as one write. The length is checked as soon as its four bytes are here, before any body byte is buffered (HP WF-1). A frame for a full
    /// held lane is refused, never blocked on (HP MD-3).
    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), Backpressured> {
        if self.closed.is_some() {
            return Ok(());
        }
        let mut rest = bytes;
        let mut refused = false;
        while !rest.is_empty() {
            let take = self.tail_need().min(rest.len());
            self.rx.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            let tail = &self.rx[self.tail_start..];
            if tail.len() < 4 {
                continue;
            }
            let len = u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]) as usize;
            // HP WF-1: a `len` over `frame_bytes`, or too short for a lane and a type, is refused before the body is read; the epoch ends.
            if len < 2 || len > self.frame_bytes as usize {
                self.refused_buffered = tail.len() - 4;
                self.close("epoch_ended");
                return Ok(());
            }
            if tail.len() < 4 + len {
                continue;
            }
            // A complete frame of a held lane moves to the bounded parking queue.
            let lane = tail[4] as usize;
            if lane < 4 && self.held[lane] {
                let chunk = tail.to_vec();
                let (frames, cap_bytes) = self.lane_caps[lane];
                let used: u64 = self.parked[lane].iter().map(|c| c.len() as u64).sum();
                if self.parked[lane].len() as u64 >= frames || used + chunk.len() as u64 > cap_bytes
                {
                    refused = true;
                } else {
                    self.parked[lane].push(chunk);
                }
                self.rx.truncate(self.tail_start);
            } else {
                self.tail_start = self.rx.len();
            }
        }
        if refused {
            Err(Backpressured)
        } else {
            Ok(())
        }
    }

    /// Removes `len` bytes of a complete frame at `start`.
    fn remove(&mut self, start: usize, len: usize) {
        self.rx.drain(start..start + len);
        self.tail_start -= len;
    }

    /// The first complete frame of each lane in the buffer, as `(lane, start, total)`. The scan stops at the first header that is not a frame (a bad length or a gap).
    fn heads(&self) -> Vec<(usize, usize, usize)> {
        let mut out: Vec<(usize, usize, usize)> = Vec::new();
        let mut at = 0;
        while self.rx.len() >= at + 6 {
            let len = u32::from_le_bytes([
                self.rx[at],
                self.rx[at + 1],
                self.rx[at + 2],
                self.rx[at + 3],
            ]) as usize;
            let total = 4 + len;
            if len < 2 || len > self.frame_bytes as usize || self.rx.len() < at + total {
                break;
            }
            let lane = (self.rx[at + 4] as usize).min(4);
            if !out.iter().any(|h| h.0 == lane) {
                out.push((lane, at, total));
            }
            at += total;
        }
        out
    }

    /// The next frame, decoded in the receive order of steward ruling R-6: length, payload form, epoch, then type.
    ///
    /// The lanes are read independently: when several lanes have a frame waiting, which one is handled next is a choice, and each lane keeps its order (HP MD-1).
    pub fn next_inbound(&mut self, ch: &mut Chooser) -> Option<Inbound> {
        loop {
            if self.closed.is_some() && self.rx.is_empty() {
                return None;
            }
            let heads = self.heads();
            // No complete frame is waiting: the tail is still arriving.
            if heads.is_empty() {
                return None;
            }
            let h = heads[ch.pick(Choice::LaneOrder, heads.len())];
            let (start, total) = (h.1, h.2);
            let mut frame: Vec<u8> = self.rx[start..start + total].to_vec();
            match decode(&frame, self.frame_bytes, self.epoch) {
                Ok(Decoded::Incomplete { .. }) => return None,
                Ok(Decoded::Frame {
                    frame: f,
                    lane,
                    used,
                }) => {
                    let payload = serde_json::from_slice(&frame[6..used]).unwrap_or(Value::Null);
                    self.remove(start, used);
                    self.seen.push(Seen {
                        lane: lane_name(lane),
                        ty: f.frame_type().spec().name,
                        id: id_of(&payload),
                    });
                    return Some(Inbound::Frame {
                        frame: Box::new(f),
                        lane,
                    });
                }
                // HP MD-4, CR-5: a stale frame is dropped and counted, whatever its type; it never gets a Fault.
                Ok(Decoded::Stale { used, .. }) => {
                    self.stale += 1;
                    self.remove(start, used);
                }
                // HP VR-2: an unknown advisory frame is ignored and counted.
                Ok(Decoded::IgnoredAdvisory { used, .. }) => {
                    self.ignored += 1;
                    self.remove(start, used);
                }
                // HP VR-2: an unknown state-changing frame is refused.
                Ok(Decoded::UnknownStateChanging { ty, used }) => {
                    self.remove(start, used);
                    return Some(Inbound::Unknown { ty });
                }
                Err(e) => match e.disposition() {
                    // HP WF-1, WF-2: the epoch ends. The receiver stops at the header: it reads no body.
                    Disposition::EndsEpoch => {
                        self.close("epoch_ended");
                        return None;
                    }
                    // HP WF-2 duplicate keys, WF-3, WF-4, R-4: the frame is refused as a malformed request and the epoch goes on.
                    Disposition::InvalidRequest => {
                        let len =
                            u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
                        frame.truncate(4 + len);
                        let payload = serde_json::from_slice(&frame[6..]).unwrap_or(Value::Null);
                        let (lane, ty) = (frame[4], frame[5]);
                        self.remove(start, 4 + len);
                        return Some(Inbound::Malformed { ty, lane, payload });
                    }
                },
            }
        }
    }

    /// A frame for the peer. The sender refuses a frame over `frame_bytes` and truncates nothing (HP MD-2).
    pub fn send(&mut self, lane: Lane, frame: &Frame) -> Result<(), EncodeError> {
        let bytes = encode(lane, frame, self.frame_bytes)?;
        self.out[lane.byte() as usize].push_back(bytes);
        Ok(())
    }

    /// Complete wire bytes for the peer, unchanged (a fault injection: nothing checks them).
    pub fn send_wire(&mut self, lane: Lane, bytes: Vec<u8>) {
        self.out[lane.byte() as usize].push_back(bytes);
    }

    /// The next write for the peer. Frames of one lane keep their send order; the lanes interleave by choice, and frames batch into writes by choice (HP MD-1).
    pub fn read(&mut self, ch: &mut Chooser) -> Option<Vec<u8>> {
        let mut batch: Vec<u8> = Vec::new();
        loop {
            let live: Vec<usize> = (0..4).filter(|i| !self.out[*i].is_empty()).collect();
            if live.is_empty() {
                break;
            }
            let lane = live[ch.pick(Choice::LaneOrder, live.len())];
            batch.extend(self.out[lane].pop_front().expect("a live lane has a frame"));
            if ch.pick(Choice::Batching, 2) == 0 {
                self.ready.push_back(std::mem::take(&mut batch));
            }
        }
        if !batch.is_empty() {
            self.ready.push_back(batch);
        }
        self.ready.pop_front()
    }

    /// The `received` observation: the frames the receiver was handed on one lane, in delivery order (HP MD-1).
    pub fn received(&self, lane: &str) -> Value {
        Value::Array(
            self.seen
                .iter()
                .filter(|s| s.lane == lane)
                .map(|s| json!({"type": s.ty, "id": s.id}))
                .collect(),
        )
    }
}

fn id_of(payload: &Value) -> Value {
    payload
        .get("call_id")
        .or_else(|| payload.get("request_id"))
        .cloned()
        .unwrap_or(Value::Null)
}

/// A lenient read of the id of a malformed request, so that the refusal can name it (HP WF-2, WF-3, WF-4).
pub fn lenient_id(payload: &Value, key: &str) -> Option<WireId> {
    serde_json::from_value(payload.get(key)?.clone()).ok()
}

impl Link {
    /// True when a complete frame is waiting in the receive buffer (written and not yet processed).
    pub fn has_frame(&self) -> bool {
        self.rx.len() >= 4
            && self.rx.len()
                >= 4 + u32::from_le_bytes([self.rx[0], self.rx[1], self.rx[2], self.rx[3]]) as usize
    }
}
