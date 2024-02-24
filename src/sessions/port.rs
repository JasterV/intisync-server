use serde::{Deserialize, Serialize};
use std::fmt::Display;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum SessionState {
    WaitingForController,
    InProgress,
}

impl Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WaitingForController => write!(f, "waiting_for_controller"),
            Self::InProgress => write!(f, "in_progress"),
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error("Failed to parse SessionState from a string: '{0}'")]
pub struct ParseSessionStateError(String);

impl TryFrom<String> for SessionState {
    type Error = ParseSessionStateError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.trim() {
            "waiting_for_controller" => Ok(Self::WaitingForController),
            "in_progress" => Ok(Self::InProgress),
            _ => Err(ParseSessionStateError(s)),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum CreateSessionError {
    #[error("Attempted to create session with already existing ID")]
    UnexpectedSessionIdAlreadyInUse(Uuid),
    #[error("Failed to create session: '{0}'")]
    IoError(#[from] anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum UpdateSessionStateError {
    #[error("Session with id '{0}' does not exist")]
    UnknownSession(Uuid),
    #[error("Failed to update session: '{0}'")]
    IoError(#[from] anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum DeleteSessionError {
    #[error("Failed to delete session: '{0}'")]
    IoError(#[from] anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum GetSessionStateError {
    #[error("Failed to get the session state: '{0}'")]
    IoError(#[from] anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum ExistsSessionError {
    #[error("Failed to check if a session exists: '{0}'")]
    IoError(#[from] anyhow::Error),
}

#[cfg_attr(test, mockall::automock)]
pub trait SessionStore: Send + Sync {
    fn create_session(
        &self,
    ) -> impl std::future::Future<Output = Result<Uuid, CreateSessionError>> + std::marker::Send;

    fn delete_session(
        &self,
        id: Uuid,
    ) -> impl std::future::Future<Output = Result<(), DeleteSessionError>> + std::marker::Send;

    fn session_state(
        &self,
        id: Uuid,
    ) -> impl std::future::Future<Output = Result<Option<SessionState>, GetSessionStateError>>
           + std::marker::Send;

    fn exists_session(
        &self,
        id: Uuid,
    ) -> impl std::future::Future<Output = Result<bool, ExistsSessionError>> + std::marker::Send;

    fn update_session_state(
        &self,
        id: Uuid,
        state: SessionState,
    ) -> impl std::future::Future<Output = Result<(), UpdateSessionStateError>> + std::marker::Send;
}
