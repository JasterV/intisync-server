use crate::socket::port::MessageWithAck;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct JoinSessionRequest {
    pub session_id: Uuid,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(Debug, PartialEq, serde::Deserialize))]
pub enum JoinSessionErrorKind {
    AlreadyInASession,
    SessionNotFound,
    SessionFull,
    ServerError,
    HubResponseTimeout,
    Rejected,
}

#[derive(Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(Debug, PartialEq, serde::Deserialize))]
pub enum JoinSessionResponse {
    Error { kind: JoinSessionErrorKind },
    Ok {},
}

impl JoinSessionResponse {
    pub fn with_err(kind: JoinSessionErrorKind) -> Self {
        Self::Error { kind }
    }
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug, PartialEq, Default))]
pub struct JoinSessionPermissionRequest {
    pub message: String,
}

#[derive(Deserialize, Debug)]
#[cfg_attr(test, derive(PartialEq, Default))]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum JoinSessionPermissionResponse {
    #[cfg_attr(test, default)]
    Accept,
    Reject,
}

impl MessageWithAck for JoinSessionPermissionRequest {
    type Ack = JoinSessionPermissionResponse;
}

// TODO: Refactor this command once we know what is the preferred way clients should receive
// vibration data
#[derive(Serialize, Deserialize)]
pub struct VibrateCmd {
    pub value: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerErrorKind {
    Permissions,
    VibrateCmdSendError,
}

#[derive(Serialize)]
pub struct ControllerErrorMsg {
    kind: ControllerErrorKind,
    message: String,
}

impl ControllerErrorMsg {
    pub fn new(kind: ControllerErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::actors::controller::{ControllerErrorKind, ControllerErrorMsg};

    use super::{
        JoinSessionErrorKind, JoinSessionPermissionRequest, JoinSessionPermissionResponse,
        JoinSessionRequest, JoinSessionResponse, VibrateCmd,
    };
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn test_serialize_join_session_request() {
        let request = JoinSessionRequest {
            session_id: Uuid::nil(),
            message: "hello world".into(),
        };

        let serialized = format!(
            r#"{{"message":"hello world","session_id":"{}"}}"#,
            Uuid::nil()
        );

        assert_eq!(
            serde_json::from_str::<JoinSessionRequest>(&serialized).unwrap(),
            request
        );
    }

    #[test]
    fn test_serialize_join_session_ok_response() {
        let response = JoinSessionResponse::Ok {};
        assert_eq!(json!(response).to_string(), r#"{"type":"ok"}"#);
    }

    #[test]
    fn test_serialize_join_session_err_response() {
        let response = JoinSessionResponse::Error {
            kind: JoinSessionErrorKind::SessionFull,
        };
        assert_eq!(
            json!(response).to_string(),
            r#"{"kind":"session_full","type":"error"}"#
        )
    }

    #[test]
    fn test_serialize_join_session_permission_request() {
        let response = JoinSessionPermissionRequest {
            message: "hello!".into(),
        };
        assert_eq!(json!(response).to_string(), r#"{"message":"hello!"}"#)
    }

    #[test]
    fn test_deserialize_join_session_permission_response() {
        let serialized = r#"{"type":"accept"}"#;
        assert_eq!(
            serde_json::from_str::<JoinSessionPermissionResponse>(serialized).unwrap(),
            JoinSessionPermissionResponse::Accept
        );
        let serialized = r#"{"type":"reject"}"#;
        assert_eq!(
            serde_json::from_str::<JoinSessionPermissionResponse>(serialized).unwrap(),
            JoinSessionPermissionResponse::Reject
        );
    }

    #[test]
    fn test_serialize_vibrate_command() {
        let command = VibrateCmd { value: 12.0 };
        assert_eq!(json!(command).to_string(), r#"{"value":12.0}"#);
    }

    #[test]
    fn test_serialize_controller_error_msg() {
        let msg = ControllerErrorMsg::new(ControllerErrorKind::Permissions, "bro no");
        assert_eq!(
            json!(msg).to_string(),
            r#"{"kind":"permissions","message":"bro no"}"#
        );
    }
}
