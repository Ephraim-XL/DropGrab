//! Event bus: monotonically increasing `seq`, a 1,000-event replay window, and a live broadcast
//! (section 3.5 reconnect rules, section 14 Appendix B "Event replay window").

use std::collections::VecDeque;
use std::sync::Mutex;

use dg_types::Event;
use serde_json::Value;
use tokio::sync::broadcast;

pub const REPLAY_WINDOW: usize = 1000;

pub struct EventBus {
    inner: Mutex<BusInner>,
    tx: broadcast::Sender<Event>,
}

struct BusInner {
    next_seq: u64,
    ring: VecDeque<Event>,
}

/// Result of a replay request from a reconnecting client.
pub enum Replay {
    /// Every event after the requested seq, in order.
    Events(Vec<Event>),
    /// The requested seq fell out of the replay window: the client must reload with queries.
    Resync,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(4096);
        Self { inner: Mutex::new(BusInner { next_seq: 0, ring: VecDeque::new() }), tx }
    }

    pub fn emit(&self, kind: &str, data: Value) -> Event {
        let ev = {
            let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            inner.next_seq += 1;
            let ev = Event { seq: inner.next_seq, kind: kind.to_string(), data };
            inner.ring.push_back(ev.clone());
            while inner.ring.len() > REPLAY_WINDOW {
                inner.ring.pop_front();
            }
            ev
        };
        // A send error only means nobody is listening right now; the ring still has the event.
        let _ = self.tx.send(ev.clone());
        ev
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    /// Events with `seq > after`. `after = 0` means "from the beginning of the window".
    pub fn replay(&self, after: u64) -> Replay {
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let oldest = inner.ring.front().map(|e| e.seq);
        match oldest {
            Some(oldest) if after + 1 < oldest && after > 0 => Replay::Resync,
            _ => Replay::Events(inner.ring.iter().filter(|e| e.seq > after).cloned().collect()),
        }
    }

    pub fn last_seq(&self) -> u64 {
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).next_seq
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn seq_is_monotonic_and_replay_filters() {
        let bus = EventBus::new();
        for i in 0..5 {
            bus.emit("x", json!({ "i": i }));
        }
        match bus.replay(2) {
            Replay::Events(evs) => {
                let seqs: Vec<u64> = evs.iter().map(|e| e.seq).collect();
                assert_eq!(seqs, vec![3, 4, 5]);
            }
            Replay::Resync => panic!("unexpected resync"),
        }
    }

    #[test]
    fn replay_beyond_window_requests_resync() {
        let bus = EventBus::new();
        for _ in 0..(REPLAY_WINDOW + 10) {
            bus.emit("x", json!({}));
        }
        assert!(matches!(bus.replay(3), Replay::Resync));
        assert!(matches!(bus.replay(bus.last_seq() - 1), Replay::Events(_)));
    }
}
