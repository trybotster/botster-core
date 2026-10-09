//! The drain of the PTY at the payload's exit (`Action::DrainPty`), as one decision that a driver advances after each read.
//! The testkit edge drives it now. The real driver adopts it with audit finding A31 (#163 part B); until then the real
//! driver's drain reads only the count, without the flushing read. That is the open A31 defect: the two drains differ
//! until A31 lands (BUILD.md: a behavior that differs between them is an edge bug).

/// The drain of the PTY that `Action::DrainPty` asks for at the payload's exit: the output written before the exit is read
/// before the exit is reported (EV-4, ST-5), and output that a remaining process of the group writes later cannot hold the
/// exit back.
///
/// The count of the PTY (`FIONREAD`) can leave out output that the terminal has not moved to its read buffer yet. A
/// nonblocking read moves it before it reports that no byte is left. So the drain reads the count, then reads until a read
/// finds nothing or the end of the output. If that flushing read finds bytes, the drain reads at most the count measured
/// once right after it, so a process that keeps writing cannot extend the drain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drain {
    /// The bytes still to read of the count taken when the drain was asked.
    Counted(usize),
    /// The count is read. The next read flushes, and it ends the drain unless it finds bytes.
    Flush,
    /// The bytes still to read of the count measured after the flushing read.
    Flushed(usize),
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
            Drain::Counted(left) | Drain::Flushed(left) => left.min(chunk),
            Drain::Flush | Drain::Done => chunk,
        }
    }

    /// The drain after a read that found `n` bytes (0: nothing, or the end of the output, which completes the drain).
    /// `measure` gives the count of the PTY; it is called only after a flushing read that found bytes.
    pub fn after_read<E>(
        self,
        n: usize,
        measure: impl FnOnce() -> Result<usize, E>,
    ) -> Result<Drain, E> {
        Ok(match self {
            _ if n == 0 => Drain::Done,
            Drain::Counted(left) if n < left => Drain::Counted(left - n),
            Drain::Counted(_) => Drain::Flush,
            Drain::Flush => match measure()? {
                0 => Drain::Done,
                left => Drain::Flushed(left),
            },
            Drain::Flushed(left) if n < left => Drain::Flushed(left - n),
            Drain::Flushed(_) | Drain::Done => Drain::Done,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads like the driver: `reads` gives the bytes that each read finds (0: nothing or the end), `measures` the count
    /// after the flushing read. Returns the bytes that the drain read and the reads it made.
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
        assert!(measures.next().is_none(), "every measure was used");
        (total, made)
    }

    /// A31: output that the count left out (still in the terminal's flip buffer) is read: after the counted bytes, the
    /// flushing read finds it, and the count measured once after that read bounds the rest.
    #[test]
    fn a_drain_reads_the_output_that_its_count_left_out() {
        let chunk = 8;
        // The count saw 3 bytes; 4 more were not counted. The flushing read finds 2, and the count after it the other 2.
        let reads = [3, 2, 2];
        let (total, made) = drain_with(3, &reads, &[2], chunk);
        assert_eq!(total, reads.iter().sum::<usize>());
        assert_eq!(
            made,
            reads.len(),
            "the drain ends when the measured count is read"
        );
        // A count of 0 still makes the flushing read; a count of 0 after it ends the drain.
        let reads = [2];
        let (total, made) = drain_with(0, &reads, &[0], chunk);
        assert_eq!((total, made), (2, reads.len()));
        // A flushing read that finds nothing ends the drain at once.
        assert_eq!(drain_with(3, &[3, 0], &[], chunk), (3, 2));
    }

    /// A31: a process that keeps writing cannot hold the exit back: past the count, the drain reads at most one flushing
    /// read and the count measured once after it.
    #[test]
    fn a_writer_that_keeps_writing_cannot_extend_the_drain() {
        let chunk = 8;
        let (pending, measured) = (5, 6);
        let endless = [chunk; 64];
        let (total, made) = drain_with(pending, &endless, &[measured], chunk);
        assert!(total <= pending + chunk + measured, "{total}");
        assert!(
            made < endless.len(),
            "the drain ended while the writer still wrote"
        );
    }
}
