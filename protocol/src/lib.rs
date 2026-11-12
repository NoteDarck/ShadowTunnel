use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_FRAME: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("frame exceeds maximum size")]
    TooLarge,
    #[error("invalid frame length")]
    InvalidLength,
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Auth { token: String, tunnel: String },
    AuthOk,
    AuthError,
    Heartbeat,
    HeartbeatAck,
    Open { id: u32 },
    OpenOk { id: u32 },
    OpenError { id: u32, reason: String },
    Data { id: u32, bytes: Vec<u8> },
    Close { id: u32 },
}

pub fn encode(message: &Message) -> Result<Vec<u8>, ProtocolError> {
    let payload = serde_json::to_vec(message)?;
    if payload.len() > MAX_FRAME {
        return Err(ProtocolError::TooLarge);
    }
    let length = u32::try_from(payload.len()).map_err(|_| ProtocolError::TooLarge)?;
    let mut frame = Vec::with_capacity(payload.len() + 4);
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub fn decode(frame: &[u8]) -> Result<Message, ProtocolError> {
    if frame.len() < 4 {
        return Err(ProtocolError::InvalidLength);
    }
    let length = u32::from_be_bytes(frame[..4].try_into().unwrap()) as usize;
    if length > MAX_FRAME || frame.len() != length + 4 {
        return Err(ProtocolError::InvalidLength);
    }
    Ok(serde_json::from_slice(&frame[4..])?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip() {
        let msg = Message::Data {
            id: 7,
            bytes: b"abc".to_vec(),
        };
        assert_eq!(decode(&encode(&msg).unwrap()).unwrap(), msg);
    }
    #[test]
    fn rejects_bad_length() {
        assert!(matches!(
            decode(&[0, 0, 0, 4, 1]),
            Err(ProtocolError::InvalidLength)
        ));
    }
}
