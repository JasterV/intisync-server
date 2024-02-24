use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(Debug, PartialEq, serde::Deserialize))]
pub enum StartSessionError {
    AlreadyInASession,
    ServerError,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug, PartialEq, serde::Deserialize))]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum StartSessionResponse {
    Error { kind: StartSessionError },
    Ok { session_id: Uuid },
}

impl StartSessionResponse {
    pub fn error(kind: StartSessionError) -> Self {
        Self::Error { kind }
    }
}

#[cfg(test)]
mod tests {
    use super::{StartSessionError, StartSessionResponse};
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn test_serialize_start_ression_response_ok_to_json() {
        let response = StartSessionResponse::Ok {
            session_id: Uuid::nil(),
        };
        assert_eq!(
            json!(response).to_string(),
            format!(r#"{{"session_id":"{}","type":"ok"}}"#, Uuid::nil())
        )
    }

    #[test]
    fn test_serialize_start_ression_response_err_to_json() {
        let response = StartSessionResponse::Error {
            kind: StartSessionError::AlreadyInASession,
        };
        assert_eq!(
            json!(response).to_string(),
            r#"{"kind":"already_in_a_session","type":"error"}"#
        )
    }
}
