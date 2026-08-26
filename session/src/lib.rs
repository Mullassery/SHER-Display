//! SHER-Display graphical session (spec section 33).
//!
//! A session is explicit state, not an implicit side effect of "a user is
//! logged in somewhere": `LoggedOut → Active → Locked → Active → LoggedOut`,
//! with illegal transitions rejected rather than silently coerced — the
//! same discipline the window state machine (spec v2 section 8) asks for,
//! applied here because a session bug (e.g. unlocking without a real
//! authentication step) is a security bug, not just a UX one.

use sher_common::{Error, ObjectId, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionState {
    LoggedOut,
    Active,
    Locked,
}

pub struct GraphicalSession {
    state: SessionState,
    user: Option<String>,
    /// Seats: independent (output, input-device-group) pairs — tracked
    /// regardless of login state, since a login screen can be shown on a
    /// seat before anyone has authenticated (spec section 42's
    /// multi-modal-device readiness: don't assume exactly one seat).
    seats: Vec<ObjectId>,
}

impl GraphicalSession {
    pub fn new() -> Self {
        GraphicalSession {
            state: SessionState::LoggedOut,
            user: None,
            seats: Vec::new(),
        }
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    pub fn login(&mut self, user: impl Into<String>) -> Result<()> {
        if self.state != SessionState::LoggedOut {
            return Err(Error::Security(
                "cannot log in: session is already active".to_string(),
            ));
        }
        self.user = Some(user.into());
        self.state = SessionState::Active;
        Ok(())
    }

    pub fn logout(&mut self) -> Result<()> {
        if self.state == SessionState::LoggedOut {
            return Err(Error::Security(
                "cannot log out: no active session".to_string(),
            ));
        }
        self.user = None;
        self.state = SessionState::LoggedOut;
        Ok(())
    }

    pub fn lock(&mut self) -> Result<()> {
        if self.state != SessionState::Active {
            return Err(Error::Security(
                "cannot lock: session is not active".to_string(),
            ));
        }
        self.state = SessionState::Locked;
        Ok(())
    }

    /// Real unlock requires a real authentication step upstream (session
    /// UI / Aurora's lock screen) — this only records the transition once
    /// that has already succeeded; it is not itself an authentication
    /// check.
    pub fn unlock(&mut self) -> Result<()> {
        if self.state != SessionState::Locked {
            return Err(Error::Security(
                "cannot unlock: session is not locked".to_string(),
            ));
        }
        self.state = SessionState::Active;
        Ok(())
    }

    /// Session restart (compositor/session process restarts) without a
    /// full logout — valid from either `Active` or `Locked`, always lands
    /// back in `Active`.
    pub fn restart(&mut self) -> Result<()> {
        if self.state == SessionState::LoggedOut {
            return Err(Error::Security(
                "cannot restart: no active session".to_string(),
            ));
        }
        self.state = SessionState::Active;
        Ok(())
    }

    pub fn add_seat(&mut self, seat_id: ObjectId) {
        if !self.seats.contains(&seat_id) {
            self.seats.push(seat_id);
        }
    }

    pub fn remove_seat(&mut self, seat_id: &ObjectId) {
        self.seats.retain(|s| s != seat_id);
    }

    pub fn seats(&self) -> &[ObjectId] {
        &self.seats
    }
}

impl Default for GraphicalSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_lifecycle_happy_path() {
        let mut session = GraphicalSession::new();
        session.login("georgi").unwrap();
        assert_eq!(session.state(), SessionState::Active);

        session.lock().unwrap();
        assert_eq!(session.state(), SessionState::Locked);
        // user stays associated with the session while locked
        assert_eq!(session.user(), Some("georgi"));

        session.unlock().unwrap();
        assert_eq!(session.state(), SessionState::Active);

        session.logout().unwrap();
        assert_eq!(session.state(), SessionState::LoggedOut);
        assert_eq!(session.user(), None);
    }

    #[test]
    fn cannot_unlock_a_session_that_is_not_locked() {
        let mut session = GraphicalSession::new();
        session.login("georgi").unwrap();
        assert!(session.unlock().is_err());
    }

    #[test]
    fn cannot_lock_before_logging_in() {
        let mut session = GraphicalSession::new();
        assert!(session.lock().is_err());
    }

    #[test]
    fn cannot_log_in_twice() {
        let mut session = GraphicalSession::new();
        session.login("georgi").unwrap();
        assert!(session.login("someone_else").is_err());
    }

    #[test]
    fn restart_preserves_login_and_returns_to_active() {
        let mut session = GraphicalSession::new();
        session.login("georgi").unwrap();
        session.lock().unwrap();

        session.restart().unwrap();

        assert_eq!(session.state(), SessionState::Active);
        assert_eq!(session.user(), Some("georgi"));
    }

    #[test]
    fn seats_survive_logout() {
        let mut session = GraphicalSession::new();
        let seat = ObjectId::new();
        session.add_seat(seat);
        session.login("georgi").unwrap();
        session.logout().unwrap();

        assert_eq!(session.seats(), &[seat]);
    }
}
