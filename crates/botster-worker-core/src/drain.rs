//! The drain of the PTY at the payload's exit (`Action::DrainPty`), as one decision that a driver advances after each read.
//! Both drivers drive it: the testkit edge and the real driver (audit finding A31, R-107, R-108), so the two drains are the
//! same (BUILD.md: a behavior that differs between them is an edge bug).

/// The drain of the PTY that `Action::DrainPty` asks for at the payload's exit: the output written before the exit is read
/// before the exit is reported (OU-7 "lossless", EV-4, ST-5).
///
/// The count of the PTY (`FIONREAD`) can leave out output that the terminal has not moved to its read buffer yet: on Linux
/// it counts only the line discipline's read buffer (at most 4096 bytes), and more output waits in the flip buffers. A
/// nonblocking read moves that output before it reports that no byte is left. So the drain repeats a counted part and then
/// a flushing read, and it ends only when a read finds nothing or the output ends (`EIO` on Linux, EOF on macOS). A count is
/// only the size of the next reads, never a reason to stop: the drain has no byte bound (R-108 (1), (2)).
///
/// A process that holds the PTY and keeps writing after the payload's exit keeps the drain going. The contract does not say
/// when such a tail ends (R-108 (3), Core Amendment 24), so the drain does not stop it early.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drain {
    /// The bytes still to read of the last count.
    Counted(usize),
    /// The count is read. The next read flushes: it ends the drain if it finds nothing.
    Flush,
    /// The drain is complete: `PtyDrained` is due.
    Done,
}

impl Drain {
    /// A drain of the PTY that holds `pending` bytes now.
    pub fn asked(pending: usize) -> Drain {
        match pending {
            0 => Drain::Flush,
            left => Drain::Counted(left),
        }
    }

    /// The most bytes that the next read takes, within the driver's read bound `chunk`.
    pub fn want(self, chunk: usize) -> usize {
        match self {
            Drain::Counted(left) => left.min(chunk),
            Drain::Flush | Drain::Done => chunk,
        }
    }

    /// The drain after a read that found `n` bytes (0: nothing, or the end of the output, which completes the drain).
    /// `measure` gives the count of the PTY; it is called only after a flushing read that found bytes, and it gives the size
    /// of the next counted part.
    pub fn after_read<E>(
        self,
        n: usize,
        measure: impl FnOnce() -> Result<usize, E>,
    ) -> Result<Drain, E> {
        Ok(match self {
            _ if n == 0 => Drain::Done,
            Drain::Counted(left) if n < left => Drain::Counted(left - n),
            Drain::Counted(_) => Drain::Flush,
            Drain::Flush => Drain::asked(measure()?),
            Drain::Done => Drain::Done,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads like the driver: `reads` gives the bytes that each read finds (0: nothing or the end), `measures` the count
    /// after each flushing read that found bytes. Returns the bytes that the drain read and the reads it made.
    fn drain_with(
        pending: usize,
        reads: &[usize],
        measures: &[usize],
        chunk: usize,
    ) -> (usize, usize) {
        let (mut reads, mut measures) = (reads.iter(), measures.iter());
        let mut drain = Drain::asked(pending);
        let (mut total, mut made) = (0, 0);
        while drain != Drain::Done {
            let found = (*reads.next().expect("the drain read again")).min(drain.want(chunk));
            made += 1;
            total += found;
            drain = drain
                .after_read(found, || {
                    Ok::<_, ()>(*measures.next().expect("the drain measured again"))
                })
                .unwrap();
        }
        assert!(reads.next().is_none(), "every read was made");
        assert!(measures.next().is_none(), "every measure was used");
        (total, made)
    }

    /// A31, R-108 (1): output that the count left out (still in the terminal's flip buffer) is read. After the counted bytes,
    /// a flushing read finds it, the count after that read sizes the next part, and only a read that finds nothing ends the
    /// drain.
    #[test]
    fn a_drain_reads_the_output_that_its_count_left_out() {
        let chunk = 8;
        // The count saw 3 bytes; 4 more were not counted. The flushing read finds 2, the count after it the other 2, and
        // the next flushing read finds nothing.
        let reads = [3, 2, 2, 0];
        assert_eq!(drain_with(3, &reads, &[2], chunk), (7, reads.len()));
        // A count of 0 still makes the flushing read. A count of 0 after a read that found bytes is a flushing read again.
        let reads = [2, 3, 0];
        assert_eq!(drain_with(0, &reads, &[0, 0], chunk), (5, reads.len()));
        // A flushing read that finds nothing ends the drain at once.
        assert_eq!(drain_with(3, &[3, 0], &[], chunk), (3, 2));
        assert_eq!(drain_with(0, &[0], &[], chunk), (0, 1));
    }

    /// A31, R-108 (1): a tail of many times the count is read whole. The model is Linux's master: the count sees at most
    /// 4096 bytes (the line discipline's read buffer), the rest waits in the flip buffer, and a read moves the next 4096
    /// into the read buffer when it is empty.
    #[test]
    fn a_tail_of_many_counts_is_read_whole() {
        const READ_BUFFER: usize = 4096;
        for tail in [0, 1, READ_BUFFER, READ_BUFFER + 1, 16 * 1024, 60 * 1024 + 7] {
            for chunk in [1000, READ_BUFFER, 64 * 1024] {
                let (mut flip, mut buffer) = (tail, 0usize);
                let fill = |buffer: &mut usize, flip: &mut usize| {
                    let moved = (READ_BUFFER - *buffer).min(*flip);
                    *buffer += moved;
                    *flip -= moved;
                };
                fill(&mut buffer, &mut flip);
                let mut drain = Drain::asked(buffer);
                let mut read = 0;
                while drain != Drain::Done {
                    if buffer == 0 {
                        fill(&mut buffer, &mut flip);
                    }
                    let found = buffer.min(drain.want(chunk));
                    buffer -= found;
                    read += found;
                    drain = drain.after_read(found, || Ok::<_, ()>(buffer)).unwrap();
                }
                assert_eq!(read, tail, "tail {tail}, chunk {chunk}");
            }
        }
    }

    /// Each read moves the drain by exactly the bytes it found: a short read leaves the rest of the count, a read of the whole
    /// count moves to the flushing read, a flushing read that finds bytes moves to the count measured after it, and a read
    /// of 0 ends the drain.
    #[test]
    fn a_read_moves_the_drain_by_the_bytes_it_found() {
        let after = |drain: Drain, n: usize, measured: usize| {
            drain
                .after_read(n, || Ok::<_, std::convert::Infallible>(measured))
                .unwrap()
        };
        assert_eq!(after(Drain::Counted(5), 2, 9), Drain::Counted(3));
        assert_eq!(after(Drain::Counted(5), 5, 9), Drain::Flush);
        assert_eq!(after(Drain::Counted(5), 0, 9), Drain::Done);
        assert_eq!(after(Drain::Flush, 1, 4), Drain::Counted(4));
        assert_eq!(after(Drain::Flush, 1, 0), Drain::Flush);
        assert_eq!(after(Drain::Flush, 0, 4), Drain::Done);
        assert_eq!(after(Drain::Done, 3, 4), Drain::Done);
        assert_eq!(Drain::Counted(5).want(3), 3);
        assert_eq!(Drain::Counted(2).want(3), 2);
        assert_eq!(Drain::Flush.want(3), 3);
    }

    /// The count after a flushing read is read only when that read found bytes, and its failure is the drain's failure.
    #[test]
    fn only_a_flushing_read_that_found_bytes_measures_and_its_failure_is_returned() {
        let failing = || Err::<usize, &str>("count failed");
        assert_eq!(Drain::Flush.after_read(2, failing), Err("count failed"));
        assert_eq!(Drain::Flush.after_read(0, failing), Ok(Drain::Done));
        assert_eq!(Drain::Counted(5).after_read(5, failing), Ok(Drain::Flush));
    }

    /// R-108 (2), (3): the drain has no byte bound. A writer that keeps the PTY busy keeps the drain going; when that tail
    /// ends is Core Amendment 24's question, so the drain does not stop it early.
    #[test]
    fn a_writer_that_keeps_writing_keeps_the_drain_going() {
        let chunk = 8;
        let mut drain = Drain::asked(5);
        for _ in 0..1000 {
            let found = drain.want(chunk);
            drain = drain.after_read(found, || Ok::<_, ()>(chunk)).unwrap();
            assert_ne!(drain, Drain::Done, "no byte bound ends a busy drain");
        }
    }
}
