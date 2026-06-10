//! One-shot tickets for browser WebSocket authentication.
//!
//! Browsers can't set an `Authorization` header on a WebSocket upgrade,
//! and in cross-origin deployments the `SameSite=Lax` session cookie
//! doesn't ride along either — so the upgrade needs its own credential.
//! Long-lived tokens in the query string leak into proxy logs; the
//! 2026-standard answer is a short-lived, single-use ticket fetched
//! from an authenticated REST endpoint immediately before connecting
//! (`POST /api/v1/events/ticket` → `ws://…/api/v1/events?ticket=…`).
//!
//! Tickets are process-local on purpose: they are issued and consumed
//! by the same server the socket connects to, within seconds.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Ticket lifetime — long enough for the client to turn around and
/// open the socket, short enough that a logged URL is dead on arrival.
pub const EVENT_TICKET_TTL_SECS: u64 = 30;

pub struct EventTicketService {
    tickets: Mutex<HashMap<String, Instant>>,
    ttl: Duration,
}

impl EventTicketService {
    pub fn new() -> Self {
        Self::with_ttl(Duration::from_secs(EVENT_TICKET_TTL_SECS))
    }

    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            tickets: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Mint a fresh one-shot ticket (256 bits of CSPRNG, hex).
    pub fn issue(&self) -> String {
        use rand::RngCore;
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let ticket = hex::encode(bytes);
        let mut tickets = self.tickets.lock().unwrap_or_else(|e| e.into_inner());
        let ttl = self.ttl;
        tickets.retain(|_, issued| issued.elapsed() <= ttl);
        tickets.insert(ticket.clone(), Instant::now());
        ticket
    }

    /// Validate and consume. True exactly once per issued, unexpired
    /// ticket.
    pub fn consume(&self, ticket: &str) -> bool {
        let mut tickets = self.tickets.lock().unwrap_or_else(|e| e.into_inner());
        match tickets.remove(ticket) {
            Some(issued) => issued.elapsed() <= self.ttl,
            None => false,
        }
    }
}

impl Default for EventTicketService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_is_one_shot() {
        let svc = EventTicketService::new();
        let t = svc.issue();
        assert!(svc.consume(&t));
        assert!(!svc.consume(&t), "replay must fail");
    }

    #[test]
    fn unknown_ticket_is_rejected() {
        let svc = EventTicketService::new();
        assert!(!svc.consume("nope"));
    }

    #[test]
    fn expired_ticket_is_rejected() {
        let svc = EventTicketService::with_ttl(Duration::from_millis(10));
        let t = svc.issue();
        std::thread::sleep(Duration::from_millis(30));
        assert!(!svc.consume(&t));
    }
}
