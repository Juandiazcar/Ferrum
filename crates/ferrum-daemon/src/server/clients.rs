//! Client connections: accept, the peer check after the first bytes (peers of other users are
//! refused, `docs/10` §2), the handshake (`Hello` with the nonce within
//! [`HANDSHAKE_TIMEOUT`](crate::client::HANDSHAKE_TIMEOUT), version negotiation), reading
//! requests with a per-turn cap, writing replies, and dropping clients that close, break the
//! protocol or stop reading. Nothing about a client's content is logged.

use ferrum_proto::{ClientMessage, DaemonMessage, Nonce, answer_hello};
use mio::Interest;

use super::{Daemon, MAX_CLIENTS, client_token};
use crate::client::{Client, FRAMES_PER_TURN, Phase, Received};

impl Daemon {
    /// Accepts every pending connection.
    pub(super) fn accept_clients(&mut self) {
        if !std::mem::take(&mut self.accept_ready) {
            return;
        }
        loop {
            match self.ports.listener.accept() {
                Ok(Some(connection)) => self.add_client(connection),
                Ok(None) => return,
                Err(error) => {
                    tracing::warn!(%error, "accepting a connection failed");
                    return;
                }
            }
        }
    }

    fn add_client(&mut self, connection: Box<dyn ferrum_platform::ipc::Connection>) {
        let _span = tracing::debug_span!("ipc_accept").entered();
        if self.clients.len() >= MAX_CLIENTS {
            tracing::warn!("too many connections; one was closed");
            return;
        }
        let id = self.next_client;
        self.next_client += 1;
        let now = self.ports.clock.now();
        let mut client = Client::new(connection, now, std::sync::Arc::clone(&self.outbox_total));
        let interests = Interest::READABLE | Interest::WRITABLE;
        if let Err(error) =
            self.poll.registry().register(&mut client.conn, client_token(id), interests)
        {
            tracing::warn!(%error, "a connection could not be registered");
            return;
        }
        self.clients.insert(id, client);
    }

    /// Reads and handles the requests of every client with input waiting.
    pub(super) fn serve_clients(&mut self) {
        let mut ready = std::mem::take(&mut self.scratch_clients);
        ready.clear();
        ready.extend(self.clients.iter().filter(|(_, c)| c.readable).map(|(&id, _)| id));
        for &id in &ready {
            let Some(client) = self.clients.get_mut(&id) else { continue };
            if client.receive() == Received::Gone || !check_peer(client) {
                client.phase = Phase::Closing;
                client.readable = false;
                continue;
            }
            self.handle_frames(id);
        }
        self.scratch_clients = ready;
    }

    /// Handles at most [`FRAMES_PER_TURN`] whole frames; the client stays readable if more wait.
    /// A peer that closed its end still has the frames it sent handled, then it is dropped.
    fn handle_frames(&mut self, id: u64) {
        for _ in 0..FRAMES_PER_TURN {
            let Some(client) = self.clients.get_mut(&id) else { return };
            if client.phase == Phase::Closing {
                client.readable = false;
                return;
            }
            let message = match client.next_message() {
                Ok(Some(message)) => message,
                Ok(None) => {
                    client.readable = false;
                    if client.peer.closed {
                        client.phase = Phase::Closing;
                    }
                    return;
                }
                Err(error) => {
                    tracing::warn!(%error, "a client sent a malformed frame; it is disconnected");
                    client.phase = Phase::Closing;
                    return;
                }
            };
            self.handle_message(id, message);
        }
    }

    fn handle_message(&mut self, id: u64, message: ClientMessage) {
        let Some(client) = self.clients.get_mut(&id) else { return };
        match (client.phase, message) {
            (Phase::Handshake(_), ClientMessage::Hello(hello)) => {
                let _span = tracing::debug_span!("ipc_handshake").entered();
                let answer = answer_hello(&hello, &Nonce(self.config.nonce));
                let accepted = match answer {
                    DaemonMessage::Welcome { protocol_version } => Phase::Ready(protocol_version),
                    _ => Phase::Closing,
                };
                if accepted == Phase::Closing {
                    tracing::warn!("a client was refused at the handshake");
                }
                client.send(&answer);
                client.phase = accepted;
                if matches!(accepted, Phase::Ready(_)) {
                    self.send_state(id);
                }
            }
            (Phase::Ready(_), message) => self.handle_request(id, message),
            _ => {
                tracing::warn!("a client broke the handshake; it is disconnected");
                client.phase = Phase::Closing;
            }
        }
    }

    /// Closes the connections whose handshake deadline passed.
    pub(super) fn expire_handshakes(&mut self) {
        let now = self.ports.clock.now();
        for client in self.clients.values_mut() {
            if let Phase::Handshake(deadline) = client.phase
                && now >= deadline
            {
                tracing::warn!("a connection did not complete the handshake in time; closed");
                client.phase = Phase::Closing;
            }
        }
    }

    /// The earliest handshake deadline, for the poll timeout.
    pub(super) fn next_handshake_deadline(&self) -> Option<std::time::Instant> {
        self.clients
            .values()
            .filter_map(|client| match client.phase {
                Phase::Handshake(deadline) => Some(deadline),
                _ => None,
            })
            .min()
    }

    /// Writes pending output and drops the clients that are gone or done closing.
    pub(super) fn flush_clients(&mut self) {
        let mut gone = std::mem::take(&mut self.scratch_clients);
        gone.clear();
        for (&id, client) in &mut self.clients {
            let flushed = client.flush();
            // A closing client gets its last answer unless it stopped reading altogether.
            let is_finished =
                client.phase == Phase::Closing && (client.pending() == 0 || client.is_congested());
            if flushed == Received::Gone || is_finished {
                gone.push(id);
            }
        }
        for &id in &gone {
            if let Some(mut client) = self.clients.remove(&id) {
                // Deregistering a closing connection can only fail if it is already gone.
                let _deregistered = self.poll.registry().deregister(&mut client.conn);
            }
        }
        self.scratch_clients = gone;
    }
}

/// Checks the peer's user once its first bytes arrived (`docs/10` §2); false refuses it.
fn check_peer(client: &mut Client) -> bool {
    if client.peer.checked || !client.has_input() {
        return true;
    }
    client.peer.checked = true;
    match client.conn.is_peer_owner() {
        Ok(true) => true,
        Ok(false) => {
            tracing::warn!("refused a connection from another user");
            false
        }
        Err(error) => {
            tracing::warn!(%error, "the user of a connection could not be checked; refused");
            false
        }
    }
}
