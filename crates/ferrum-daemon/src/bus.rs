//! The daemon's internal event bus (`docs/02` §9): typed, in-process. In v1.0 it carries session
//! events only, which the daemon also sends to its clients; Phase 2 crates subscribe to it
//! through [`crate::Daemon::subscribe`] without touching the session code.

use ferrum_proto::{SessionEvent, SessionId};

/// Receives the events of the bus on the daemon's I/O thread; it must return quickly.
pub trait EventSubscriber: Send {
    /// A session changed.
    fn session_event(&mut self, session: SessionId, event: &SessionEvent);
}

/// Events published during one loop turn, delivered at its end.
#[derive(Default)]
pub struct EventBus {
    queue: Vec<(SessionId, SessionEvent)>,
    subscribers: Vec<Box<dyn EventSubscriber>>,
}

impl EventBus {
    /// Adds a subscriber; it gets every event published from now on.
    pub fn subscribe(&mut self, subscriber: Box<dyn EventSubscriber>) {
        self.subscribers.push(subscriber);
    }

    /// Queues an event for delivery at the end of the turn.
    pub(crate) fn publish(&mut self, session: SessionId, event: SessionEvent) {
        self.queue.push((session, event));
    }

    /// Hands the queued events to the subscribers, then to `also` (the clients), in order.
    pub(crate) fn deliver(&mut self, mut also: impl FnMut(SessionId, &SessionEvent)) {
        let mut queue = std::mem::take(&mut self.queue);
        for (session, event) in &queue {
            for subscriber in &mut self.subscribers {
                subscriber.session_event(*session, event);
            }
            also(*session, event);
        }
        queue.clear();
        self.queue = queue; // the buffer is kept for the next turn
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, PoisonError};

    use super::*;

    struct Log(Arc<Mutex<Vec<u64>>>);

    impl EventSubscriber for Log {
        fn session_event(&mut self, session: SessionId, _event: &SessionEvent) {
            self.0.lock().unwrap_or_else(PoisonError::into_inner).push(session.0);
        }
    }

    #[test]
    fn subscribers_and_clients_get_every_event_in_order() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut bus = EventBus::default();
        bus.subscribe(Box::new(Log(Arc::clone(&seen))));
        bus.publish(SessionId(1), SessionEvent::Bell);
        bus.publish(SessionId(2), SessionEvent::Activity);
        let mut clients = Vec::new();
        bus.deliver(|session, _| clients.push(session.0));
        assert_eq!(clients, [1, 2]);
        assert_eq!(*seen.lock().unwrap_or_else(PoisonError::into_inner), [1, 2]);
        bus.deliver(|_, _| clients.push(0));
        assert_eq!(clients, [1, 2], "each event is delivered once");
    }
}
