//! The operation registry: the 120 s single-step budget and the whole-turn stop's 5 s
//! confirmation.
//!
//! A *call* is one request the host made (`engine.execute`, `engine.batch`, a `ui.*` method).
//! A *step* is what the model's call runs: for a batch, each command; for a single call, the
//! call itself. The budget is per step, so a batch that runs many quick commands is not
//! cancelled for its total time (the host times each batch step with its own clock; see
//! `engine::Batch::waiting_ms`).
//!
//! Times are milliseconds from a monotonic clock the caller supplies, so the state machine is
//! testable without sleeping.

use std::collections::{BTreeMap, BTreeSet};

/// A step's budget before it is cancelled and its call answered `stopped`.
pub const STEP_BUDGET_MS: u64 = 120_000;
/// How long a whole-turn stop waits for its in-flight calls to answer before it reports
/// `stopped: false`.
pub const STOP_CONFIRM_MS: u64 = 5_000;

/// What kind of step a call is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Runs on the loop thread: it cannot be preempted (the loop is inside it).
    Sync,
    /// A background job on a worker thread: cancelled cooperatively.
    Job,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub id: u64,
    pub op: String,
    pub started_ms: u64,
    pub kind: Kind,
}

/// The answer to a whole-turn stop: whether every in-flight call answered before the deadline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopReply {
    pub stopped: bool,
    pub at: u64,
    pub elapsed_ms: u64,
    /// `Some` when `stopped` is false: why the confirmation could not be completed.
    pub reason: Option<String>,
}

#[derive(Debug)]
struct Stop {
    at: u64,
    waiting: BTreeSet<u64>,
}

/// The calls in flight and the stop requests waiting on them.
#[derive(Debug, Default)]
pub struct Ops {
    calls: BTreeMap<u64, Call>,
    stops: BTreeMap<u64, Stop>,
}

impl Ops {
    /// A call started at `now`; `op` is the turn/operation it belongs to.
    pub fn begin(&mut self, id: u64, op: impl Into<String>, kind: Kind, now: u64) {
        self.calls.insert(id, Call { id, op: op.into(), started_ms: now, kind });
    }

    /// The call answered (of its own accord, or dropped). The stop requests waiting on it no
    /// longer do.
    pub fn end(&mut self, id: u64) -> Option<Call> {
        let call = self.calls.remove(&id)?;
        for stop in self.stops.values_mut() {
            stop.waiting.remove(&id);
        }
        Some(call)
    }

    /// The calls in flight.
    pub fn calls(&self) -> impl Iterator<Item = &Call> {
        self.calls.values()
    }

    /// Stop the calls of `op` (and only them): what `ui.operations {cancel: "<op>"}` takes.
    pub fn cancel_op(&mut self, op: &str) -> Vec<Call> {
        let ids: Vec<u64> = self.calls.values().filter(|c| c.op == op).map(|c| c.id).collect();
        ids.into_iter().filter_map(|id| self.end(id)).collect()
    }

    /// Stop the whole turn: register stop request `stop_id`, and take every call in flight (the
    /// caller cancels the job; the calls' replies still come, and are what confirms the stop).
    pub fn agent_stop(&mut self, stop_id: u64, now: u64) -> Vec<Call> {
        let calls: Vec<Call> = self.calls.values().cloned().collect();
        let waiting = calls.iter().map(|c| c.id).collect();
        self.stops.insert(stop_id, Stop { at: now, waiting });
        calls
    }

    /// The stop requests still waiting for their reply.
    pub fn pending_stops(&self) -> Vec<u64> {
        self.stops.keys().copied().collect()
    }

    /// The background-job calls whose step has used its budget, taken out of the registry (the
    /// caller cancels the job and answers them). Synchronous calls cannot be preempted and are
    /// left to report `over_budget` after they return.
    pub fn tick(&mut self, now: u64) -> Vec<Call> {
        let ids: Vec<u64> =
            self.calls.values().filter(|c| c.kind == Kind::Job && now.saturating_sub(c.started_ms) >= STEP_BUDGET_MS).map(|c| c.id).collect();
        ids.into_iter().filter_map(|id| self.end(id)).collect()
    }

    /// The reply to stop request `stop_id`, once it can be given: every in-flight call has
    /// answered (`stopped: true`), or the confirmation window closed (`stopped: false`). `None`
    /// while still waiting. The stop request is taken with its reply.
    pub fn stop_reply(&mut self, stop_id: u64, now: u64) -> Option<StopReply> {
        let stop = self.stops.get(&stop_id)?;
        let elapsed = now.saturating_sub(stop.at);
        let reply = if stop.waiting.is_empty() {
            StopReply { stopped: true, at: stop.at, elapsed_ms: elapsed, reason: None }
        } else if elapsed >= STOP_CONFIRM_MS {
            StopReply { stopped: false, at: stop.at, elapsed_ms: elapsed, reason: Some("ui thread busy".into()) }
        } else {
            return None;
        };
        self.stops.remove(&stop_id);
        Some(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ops() -> Ops {
        Ops::default()
    }

    #[test]
    fn a_job_is_budgeted_at_120_seconds_and_a_sync_call_is_not() {
        let mut o = ops();
        o.begin(1, "turn/step", Kind::Job, 1_000);
        o.begin(2, "turn/step", Kind::Sync, 1_000);
        assert!(o.tick(1_000 + STEP_BUDGET_MS - 1).is_empty());
        let due = o.tick(1_000 + STEP_BUDGET_MS);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, 1);
        assert!(o.tick(1_000 + STEP_BUDGET_MS * 10).is_empty(), "a budgeted call is taken once");
        assert_eq!(o.calls().map(|c| c.id).collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn ending_a_call_takes_it_out_of_a_stop() {
        let mut o = ops();
        o.begin(1, "turn/step", Kind::Job, 0);
        o.begin(2, "turn/step", Kind::Sync, 0);
        let calls = o.agent_stop(9, 10);
        assert_eq!(calls.len(), 2);
        assert_eq!(o.stop_reply(9, 10), None);
        o.end(1);
        assert_eq!(o.stop_reply(9, 11), None);
        o.end(2);
        let reply = o.stop_reply(9, 12).expect("every call answered");
        assert!(reply.stopped);
        assert_eq!((reply.at, reply.elapsed_ms), (10, 2));
        assert_eq!(o.stop_reply(9, 13), None, "answered once");
    }

    #[test]
    fn a_stop_with_nothing_in_flight_is_confirmed_at_once() {
        let mut o = ops();
        o.agent_stop(5, 100);
        let reply = o.stop_reply(5, 100).expect("nothing to wait for");
        assert!(reply.stopped);
        assert_eq!(reply.elapsed_ms, 0);
    }

    #[test]
    fn a_stuck_call_makes_the_stop_report_not_stopped_after_five_seconds() {
        let mut o = ops();
        o.begin(1, "turn/step", Kind::Sync, 0);
        o.agent_stop(3, 1_000);
        assert_eq!(o.stop_reply(3, 1_000 + STOP_CONFIRM_MS - 1), None);
        let reply = o.stop_reply(3, 1_000 + STOP_CONFIRM_MS).expect("the window closed");
        assert!(!reply.stopped);
        assert_eq!(reply.reason.as_deref(), Some("ui thread busy"));
        assert_eq!(reply.elapsed_ms, STOP_CONFIRM_MS);
        assert_eq!(o.stop_reply(3, 9_999), None);
    }

    #[test]
    fn cancelling_one_op_leaves_the_others() {
        let mut o = ops();
        o.begin(1, "turn/a", Kind::Job, 0);
        o.begin(2, "turn/b", Kind::Sync, 0);
        let taken = o.cancel_op("turn/a");
        assert_eq!(taken.iter().map(|c| c.id).collect::<Vec<_>>(), vec![1]);
        assert_eq!(o.calls().map(|c| c.id).collect::<Vec<_>>(), vec![2]);
        assert!(o.cancel_op("turn/a").is_empty());
    }

    #[test]
    fn a_budgeted_call_also_leaves_a_pending_stop() {
        let mut o = ops();
        o.begin(1, "turn/step", Kind::Job, 0);
        o.agent_stop(4, 1_000);
        o.tick(STEP_BUDGET_MS);
        let reply = o.stop_reply(4, STEP_BUDGET_MS + 1).expect("the budgeted call answered as stopped");
        assert!(reply.stopped);
    }
}
