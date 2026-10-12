//! The process census of a test run: what the run owns, and what it left behind.

use std::collections::HashMap;
use std::process::Command;

/// One row of `ps`.
#[derive(Debug, Clone, PartialEq)]
pub struct Proc {
    pub pid: u32,
    pub ppid: u32,
    pub pgid: u32,
    pub stat: String,
    pub command: String,
}

impl Proc {
    /// A zombie has exited and waits for its parent to reap it. On Darwin its `command` reads `<defunct>`
    /// (or the name in parentheses), so ownership never depends on the command: only `ppid`, `pgid` and the run's own record count.
    pub fn is_zombie(&self) -> bool {
        self.stat.starts_with('Z')
    }
}

/// `ps -A -ww -o pid=,ppid=,pgid=,stat=,command=` output.
pub fn parse_ps(text: &str) -> Vec<Proc> {
    text.lines()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let pid = it.next()?.parse().ok()?;
            let ppid = it.next()?.parse().ok()?;
            let pgid = it.next()?.parse().ok()?;
            let stat = it.next()?.to_string();
            let command = it.collect::<Vec<_>>().join(" ");
            Some(Proc {
                pid,
                ppid,
                pgid,
                stat,
                command,
            })
        })
        .collect()
}

/// A snapshot of every process, from `ps`.
pub fn ps_snapshot() -> Result<Vec<Proc>, String> {
    let out = Command::new("ps")
        .args(["-A", "-ww", "-o", "pid=,ppid=,pgid=,stat=,command="])
        .output()
        .map_err(|e| format!("run ps: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "ps failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(parse_ps(&String::from_utf8_lossy(&out.stdout)))
}

/// Ownership of a process by the run. A process belongs to the run when the tracker saw it
///  - be the root (the process that the xtask started, `root`), or
///  - have a parent that belongs to the run (a zombie is attributed by this rule: its `ppid` is the reaper that has not reaped it), or
///  - be in a process group of a process that belongs to the run (nextest gives each test its own group), or
///  - be in the group of a test process that the run wrapper recorded (`test_pids`; the wrapper writes the pid before the test starts, so a test that ends in a few ms is still known).
///
/// A child that calls `setsid` leaves every group of the run. The tracker owns it only when a snapshot saw it while its parent was
/// owned (then `owned` keeps it by pid, whatever its group or parent becomes). `tests/slow.rs` is the positive control.
///
/// This never looks at age or at the names of other processes. A process that the tracker did
/// not see as a descendant is not owned, so it is never reported and never killed.
/// `owned` keeps a process while it is alive or a zombie; a pid that disappears is dropped, so a reused pid is not inherited.
pub fn update_owned(owned: &mut HashMap<u32, Proc>, procs: &[Proc], root: u32, test_pids: &[u32]) {
    let alive: HashMap<u32, &Proc> = procs.iter().map(|p| (p.pid, p)).collect();
    owned.retain(|pid, _| alive.contains_key(pid));
    for (pid, p) in owned.iter_mut() {
        *p = alive[pid].clone();
    }
    loop {
        let mut groups: Vec<u32> = owned.values().map(|p| p.pgid).collect();
        groups.push(root);
        groups.extend_from_slice(test_pids);
        let found: Vec<&Proc> = procs
            .iter()
            .filter(|p| !owned.contains_key(&p.pid))
            .filter(|p| p.pid == root || owned.contains_key(&p.ppid) || groups.contains(&p.pgid))
            .collect();
        if found.is_empty() {
            return;
        }
        for p in found {
            owned.insert(p.pid, p.clone());
        }
    }
}

/// The owned processes that are still in the process table, sorted by pid. A zombie is a leftover: it proves that
/// a test left a child that nobody waited for, and it holds a process-table entry until its parent reaps it.
pub fn leftovers(owned: &HashMap<u32, Proc>) -> Vec<&Proc> {
    let mut left: Vec<&Proc> = owned.values().collect();
    left.sort_by_key(|p| p.pid);
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: u32, ppid: u32, pgid: u32, stat: &str) -> Proc {
        Proc {
            pid,
            ppid,
            pgid,
            stat: stat.into(),
            command: format!("cmd{pid}"),
        }
    }

    #[test]
    fn ps_lines_parse_with_a_spaced_command() {
        let procs = parse_ps("  12 1  12 S     sleep 31\n 13 12 12 Z <defunct>\n");
        assert_eq!(procs[0].command, "sleep 31");
        assert_eq!(procs[1].ppid, 12);
        assert!(procs[1].is_zombie());
        assert_eq!(procs.len(), 2);
    }

    // BUILD.md Testing rule 10: a child that outlives its test is a leftover, and an unrelated orphan is not.
    #[test]
    fn a_test_child_that_outlives_its_test_is_owned_and_an_unrelated_orphan_is_not() {
        let mut owned = HashMap::new();
        // root 10 (the xtask group), a test 20 in its own group, and its child 30.
        let during = vec![
            proc(5, 1, 5, "S"),
            proc(10, 5, 10, "S"),
            proc(20, 10, 20, "S"),
            proc(30, 20, 20, "S"),
            proc(99, 1, 99, "S"),
        ];
        update_owned(&mut owned, &during, 10, &[]);
        // The test and the root exit; the child is re-parented. An unrelated orphan appears, with a dead group leader.
        let after = vec![
            proc(5, 1, 5, "S"),
            proc(30, 1, 20, "S"),
            proc(31, 1, 20, "S"),
            proc(98, 1, 90, "S"),
            proc(99, 1, 99, "S"),
        ];
        update_owned(&mut owned, &after, 10, &[]);
        let pids: Vec<u32> = leftovers(&owned).iter().map(|p| p.pid).collect();
        assert_eq!(pids, vec![30, 31]);
    }

    // A zombie of an owned parent is a leftover. Its command reads `<defunct>`, so only the ppid attributes it, and a
    // zombie that the tracker never saw alive is still attributed.
    #[test]
    fn a_zombie_is_a_leftover_attributed_by_its_parent_and_a_gone_pid_is_dropped() {
        let mut owned = HashMap::new();
        update_owned(
            &mut owned,
            &[proc(10, 5, 10, "S"), proc(30, 10, 10, "S")],
            10,
            &[],
        );
        let zombie = |pid, ppid, pgid| Proc {
            command: "<defunct>".into(),
            ..proc(pid, ppid, pgid, "Z")
        };
        update_owned(
            &mut owned,
            &[proc(10, 5, 10, "S"), zombie(30, 10, 10)],
            10,
            &[],
        );
        assert_eq!(
            leftovers(&owned).iter().filter(|p| p.is_zombie()).count(),
            1
        );
        // A zombie whose group leader is gone and that was never seen alive: attributed by its owned parent (pid 10).
        update_owned(
            &mut owned,
            &[proc(10, 5, 10, "S"), zombie(40, 10, 70)],
            10,
            &[],
        );
        let pids: Vec<u32> = leftovers(&owned).iter().map(|p| p.pid).collect();
        assert_eq!(pids, vec![10, 40]);
        // A zombie of an unrelated parent is not owned.
        update_owned(&mut owned, &[zombie(50, 77, 77)], 10, &[]);
        assert!(leftovers(&owned).is_empty());
        // pid 30 is gone; a new process with that pid and no link to the run is not owned.
        update_owned(&mut owned, &[proc(30, 1, 77, "S")], 10, &[]);
        assert!(leftovers(&owned).is_empty());
    }

    #[test]
    fn a_child_of_a_recorded_test_is_owned_though_the_tracker_never_saw_the_test() {
        let mut owned = HashMap::new();
        // The test (pid 20) lived for a few ms. Its child 30 is orphaned in group 20. Process 31 is unrelated.
        let after = vec![proc(30, 1, 20, "S"), proc(31, 1, 31, "S")];
        update_owned(&mut owned, &after, 10, &[20]);
        let pids: Vec<u32> = leftovers(&owned).iter().map(|p| p.pid).collect();
        assert_eq!(pids, vec![30]);
    }
}
