//! The `Sim` (plan 4.1): "choose a ready input, hand it to its machine, route its actions".
//!
//! A `Sim` owns its machines (as [`Node`]s), the virtual clock, and the seeded scheduler. A machine registers through a
//! [`Binding`], which reaches the world only through the edge traits: it names the ready inputs of its in-memory edges (plan
//! 2.5 rule 8) and performs the machine's actions on them. Nothing in the `Sim` knows a machine's input or action type.
//!
//! Until the `HostEngine` (P1) and the `Worker` (P3) exist, the tests of this module drive test-only machines.

use crate::scheduler::SchedulerHandle;
use botster_core_edges::Machine;
use std::fmt::Debug;
use std::time::{Duration, Instant};

/// What one handled input did: the input and the actions that the machine took, as text for the trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handled {
    pub input: String,
    pub actions: Vec<String>,
}

/// One entry of the trace: which node handled which input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEntry {
    pub node: NodeId,
    pub handled: Handled,
}

/// The position of a node in registration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId(pub usize);

/// A machine with its edges, as the `Sim` sees it.
pub trait Node {
    /// How many inputs are ready at `now`: an edge endpoint whose flag matches its interest, and a due deadline.
    fn ready(&mut self, now: Instant) -> usize;
    /// Hands the `index`-th ready input (of `ready(now)`) to the machine and routes its actions.
    fn run(&mut self, now: Instant, index: usize) -> Handled;
    fn next_deadline(&self) -> Option<Instant>;
}

/// The edges of one machine (plan 2.3): where its inputs come from and where its actions go.
pub trait Binding<M: Machine> {
    /// How many edge inputs are ready now. An endpoint is ready only when a flag matches an interest that the rules of plan
    /// 2.5 allow; the binding sets the interest from the machine's state, so a parked machine reads nothing (rule 7).
    fn ready(&mut self, now: Instant, machine: &M) -> usize;
    /// Takes the `index`-th ready edge input (`index < ready`), reading the edge.
    fn take(&mut self, now: Instant, machine: &M, index: usize) -> M::Input;
    /// The input that reports a due deadline (a timer input).
    fn timer(&mut self, now: Instant) -> M::Input;
    /// Performs one action of the machine on the edges.
    fn perform(&mut self, now: Instant, action: M::Action);
}

/// A machine and its binding. A due deadline is one more ready input, after the edge inputs (Core TM-3).
pub struct MachineNode<M: Machine, B> {
    machine: M,
    binding: B,
    /// The number of edge inputs counted by the last `ready` call.
    edge_inputs: usize,
}

impl<M: Machine, B: Binding<M>> MachineNode<M, B> {
    pub fn new(machine: M, binding: B) -> MachineNode<M, B> {
        MachineNode {
            machine,
            binding,
            edge_inputs: 0,
        }
    }

    fn deadline_due(&self, now: Instant) -> bool {
        self.machine.next_deadline().is_some_and(|d| d <= now)
    }
}

impl<M, B> Node for MachineNode<M, B>
where
    M: Machine,
    M::Input: Debug,
    M::Action: Debug,
    B: Binding<M>,
{
    fn ready(&mut self, now: Instant) -> usize {
        self.edge_inputs = self.binding.ready(now, &self.machine);
        self.edge_inputs + usize::from(self.deadline_due(now))
    }

    fn run(&mut self, now: Instant, index: usize) -> Handled {
        let input = if index < self.edge_inputs {
            self.binding.take(now, &self.machine, index)
        } else {
            self.binding.timer(now)
        };
        let text = format!("{input:?}");
        self.machine.handle(now, input);
        let mut actions = Vec::new();
        while let Some(action) = self.machine.poll_action() {
            actions.push(format!("{action:?}"));
            self.binding.perform(now, action);
        }
        Handled {
            input: text,
            actions,
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.machine.next_deadline()
    }
}

/// `run_until_idle` handled `limit` inputs and work was still ready: a machine that makes work for itself without end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Livelock {
    pub limit: usize,
}

/// The simulation.
pub struct Sim {
    now: Instant,
    scheduler: SchedulerHandle,
    nodes: Vec<Box<dyn Node>>,
    trace: Vec<TraceEntry>,
}

impl Sim {
    /// A `Sim` whose virtual clock starts at `start` and whose scheduler is seeded by `seed` (Core A5-2, `with_seed`).
    pub fn with_seed(seed: u64, start: Instant) -> Sim {
        Sim {
            now: start,
            scheduler: SchedulerHandle::with_seed(seed),
            nodes: Vec::new(),
            trace: Vec::new(),
        }
    }

    /// The scheduler, for the edges that make their own choices (read and write sizes, spurious wakes).
    pub fn scheduler(&self) -> SchedulerHandle {
        self.scheduler.clone()
    }

    pub fn add(&mut self, node: Box<dyn Node>) -> NodeId {
        self.nodes.push(node);
        NodeId(self.nodes.len() - 1)
    }

    /// The virtual clock: the latest `now` that the host passed, or the latest `advance_to`.
    ///
    /// Clause: Core TM-1.
    pub fn now(&self) -> Instant {
        self.now
    }

    /// Moves the clock to `now`. The clock never goes back (TM-1: the host's `now` is monotonic for the machine).
    pub fn advance_to(&mut self, now: Instant) {
        self.now = self.now.max(now);
    }

    pub fn advance_by(&mut self, by: Duration) {
        self.now += by;
    }

    /// The earliest deadline of any machine.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.nodes.iter().filter_map(|n| n.next_deadline()).min()
    }

    /// Chooses one ready input (A5-2, OR-3), hands it to its machine and routes its actions. Returns false when no input is ready.
    pub fn step(&mut self) -> bool {
        let counts: Vec<usize> = self.nodes.iter_mut().map(|n| n.ready(self.now)).collect();
        let total: usize = counts.iter().sum();
        if total == 0 {
            return false;
        }
        // The ready inputs are listed in registration order, so index 0 is the input that no variation would run.
        let mut chosen = self.scheduler.with(|s| s.ready_work(total));
        for (node, count) in counts.iter().enumerate() {
            if chosen < *count {
                let handled = self.nodes[node].run(self.now, chosen);
                self.trace.push(TraceEntry {
                    node: NodeId(node),
                    handled,
                });
                return true;
            }
            chosen -= count;
        }
        unreachable!("the index is below the total")
    }

    /// Handles inputs until none is ready; returns how many were handled.
    pub fn run_until_idle(&mut self, limit: usize) -> Result<usize, Livelock> {
        for handled in 0..limit {
            if !self.step() {
                return Ok(handled);
            }
        }
        Err(Livelock { limit })
    }

    /// The handled inputs, in order, with their actions.
    pub fn trace(&self) -> &[TraceEntry] {
        &self.trace
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{link_pair, Interest, LinkEnd};
    use botster_core_edges::Link;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// A test-only machine: it adds the bytes that it reads and reports the running total, and it stops reading at `limit`
    /// (a bounded receive buffer, plan 2.5 rule 7).
    struct Counter {
        total: u32,
        limit: u32,
        actions: VecDeque<u32>,
    }

    #[derive(Debug)]
    enum CounterInput {
        Byte(u8),
        Timer,
    }

    impl Machine for Counter {
        type Input = CounterInput;
        type Action = u32;
        fn handle(&mut self, _now: Instant, input: CounterInput) {
            if let CounterInput::Byte(b) = input {
                self.total += u32::from(b);
                self.actions.push_back(self.total);
            }
        }
        fn poll_action(&mut self) -> Option<u32> {
            self.actions.pop_front()
        }
        fn next_deadline(&self) -> Option<Instant> {
            None
        }
    }

    struct CounterEdges {
        link: LinkEnd,
        out: Arc<Mutex<Vec<(usize, u32)>>>,
        id: usize,
    }

    impl Binding<Counter> for CounterEdges {
        fn ready(&mut self, _now: Instant, machine: &Counter) -> usize {
            // Read interest only while the machine can take a byte.
            self.link.end().set_interest(Interest {
                read: machine.total < machine.limit,
                write: false,
            });
            usize::from(self.link.is_ready())
        }
        fn take(&mut self, _now: Instant, _machine: &Counter, _index: usize) -> CounterInput {
            let mut byte = [0u8];
            assert_eq!(self.link.recv(&mut byte).unwrap(), 1);
            CounterInput::Byte(byte[0])
        }
        fn timer(&mut self, _now: Instant) -> CounterInput {
            CounterInput::Timer
        }
        fn perform(&mut self, _now: Instant, action: u32) {
            self.out.lock().unwrap().push((self.id, action));
        }
    }

    type Log = Arc<Mutex<Vec<(usize, u32)>>>;

    /// Three counters, each with its own link and queued bytes.
    fn world(seed: u64, limit: u32) -> (Sim, Log, Vec<LinkEnd>) {
        let mut sim = Sim::with_seed(seed, Instant::now());
        let log: Log = Arc::default();
        let mut feeders = Vec::new();
        for id in 0..3 {
            let (feeder, worker) = link_pair(16);
            feeders.push(feeder);
            let machine = Counter {
                total: 0,
                limit,
                actions: VecDeque::new(),
            };
            let edges = CounterEdges {
                link: worker,
                out: Arc::clone(&log),
                id,
            };
            sim.add(Box::new(MachineNode::new(machine, edges)));
        }
        (sim, log, feeders)
    }

    fn feed(feeders: &mut [LinkEnd]) {
        for (i, f) in feeders.iter_mut().enumerate() {
            f.send(&[1, 2, 3, 4].map(|b| b + i as u8)).unwrap();
        }
    }

    fn run(seed: u64) -> (Vec<TraceEntry>, Vec<(usize, u32)>) {
        let (mut sim, log, mut feeders) = world(seed, u32::MAX);
        feed(&mut feeders);
        assert_eq!(sim.run_until_idle(100).unwrap(), 12);
        let actions = log.lock().unwrap().clone();
        (sim.trace().to_vec(), actions)
    }

    /// A5-1: the same seed, script and inputs give the same order of handled inputs and the same actions.
    #[test]
    fn the_same_seed_gives_the_same_order_and_actions() {
        assert_eq!(run(5), run(5));
    }

    /// A5-2: different seeds vary only the order of the ready work. Every node handles its own inputs in order, and the
    /// per-node actions are the same under every seed.
    #[test]
    fn different_seeds_vary_only_the_order() {
        let (base_trace, base_actions) = run(0);
        let per_node = |actions: &[(usize, u32)], node| -> Vec<u32> {
            actions
                .iter()
                .filter(|(n, _)| *n == node)
                .map(|(_, a)| *a)
                .collect()
        };
        let mut orders = std::collections::BTreeSet::new();
        for seed in 0..12 {
            let (trace, actions) = run(seed);
            orders.insert(trace.iter().map(|t| t.node).collect::<Vec<_>>());
            for node in 0..3 {
                assert_eq!(
                    per_node(&actions, node),
                    per_node(&base_actions, node),
                    "seed {seed}"
                );
            }
            assert_eq!(trace.len(), base_trace.len());
        }
        assert!(
            orders.len() > 1,
            "the seed must vary the order across nodes"
        );
    }

    /// Plan 2.5 rules 7 and 8: a machine that removes its read interest is not ready work, so the `Sim` does not spin on it
    /// and does not read past what it can take. The bytes stay in the link.
    #[test]
    fn a_parked_machine_is_not_ready_work() {
        let (mut sim, log, mut feeders) = world(0, 3);
        feed(&mut feeders);
        // Node 0 reads 1, 2 (total 3, parked); node 1 reads 2 (total 2) then 3 (total 5); node 2 reads 3 (3).
        assert_eq!(sim.run_until_idle(100).unwrap(), 2 + 2 + 1);
        assert!(!sim.step(), "idle while the bytes wait");
        assert_eq!(log.lock().unwrap().len(), 5);
    }

    /// Core TM-1, TM-3: a deadline is ready work only when the clock reaches it; the clock never goes back; the due deadline
    /// is handled once, in its pump.
    #[test]
    fn a_deadline_is_work_when_the_clock_reaches_it() {
        struct Alarm {
            at: Option<Instant>,
            fired: VecDeque<&'static str>,
        }
        impl Machine for Alarm {
            type Input = &'static str;
            type Action = &'static str;
            fn handle(&mut self, _now: Instant, input: &'static str) {
                self.at = None;
                self.fired.push_back(input);
            }
            fn poll_action(&mut self) -> Option<&'static str> {
                self.fired.pop_front()
            }
            fn next_deadline(&self) -> Option<Instant> {
                self.at
            }
        }
        struct NoEdges(Arc<Mutex<Vec<&'static str>>>);
        impl Binding<Alarm> for NoEdges {
            fn ready(&mut self, _: Instant, _: &Alarm) -> usize {
                0
            }
            fn take(&mut self, _: Instant, _: &Alarm, _: usize) -> &'static str {
                unreachable!("no edge input")
            }
            fn timer(&mut self, _: Instant) -> &'static str {
                "timer"
            }
            fn perform(&mut self, _: Instant, action: &'static str) {
                self.0.lock().unwrap().push(action);
            }
        }
        let start = Instant::now();
        let done = Arc::default();
        let mut sim = Sim::with_seed(0, start);
        let at = start + Duration::from_secs(10);
        sim.add(Box::new(MachineNode::new(
            Alarm {
                at: Some(at),
                fired: VecDeque::new(),
            },
            NoEdges(Arc::clone(&done)),
        )));
        assert_eq!(sim.next_deadline(), Some(at));
        assert!(!sim.step());
        sim.advance_to(at - Duration::from_secs(1));
        assert!(!sim.step());
        sim.advance_to(at);
        sim.advance_to(start);
        assert_eq!(sim.now(), at, "the clock never goes back");
        assert!(sim.step());
        assert!(!sim.step(), "handled once");
        assert_eq!(*done.lock().unwrap(), ["timer"]);
        assert_eq!(sim.next_deadline(), None);
    }

    /// Work still ready after `limit` handled inputs is reported as a livelock with its limit.
    #[test]
    fn work_left_after_the_limit_is_a_livelock() {
        let (mut sim, _log, mut feeders) = world(0, u32::MAX);
        feed(&mut feeders);
        assert_eq!(sim.run_until_idle(3), Err(Livelock { limit: 3 }));
        assert_eq!(sim.trace().len(), 3);
    }
}
