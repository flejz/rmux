//! Length-prefixed bincode framing for detached RPC traffic.

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::RmuxError;

/// Default maximum detached frame payload length in bytes.
pub const DEFAULT_MAX_FRAME_LENGTH: usize = 1024 * 1024;

/// Encodes a detached message as a length-prefixed bincode frame.
pub fn encode_frame<T>(value: &T) -> Result<Vec<u8>, RmuxError>
where
    T: Serialize,
{
    let payload =
        bincode::serialize(value).map_err(|error| RmuxError::Encode(error.to_string()))?;

    if payload.is_empty() {
        return Err(RmuxError::EmptyFrame);
    }

    if payload.len() > DEFAULT_MAX_FRAME_LENGTH {
        return Err(RmuxError::FrameTooLarge {
            length: payload.len(),
            maximum: DEFAULT_MAX_FRAME_LENGTH,
        });
    }

    let frame_length = u32::try_from(payload.len()).map_err(|_| RmuxError::FrameTooLarge {
        length: payload.len(),
        maximum: u32::MAX as usize,
    })?;

    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&frame_length.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

/// Decodes a full detached frame in one shot.
pub fn decode_frame<T>(frame: &[u8]) -> Result<T, RmuxError>
where
    T: DeserializeOwned,
{
    if frame.len() < 4 {
        return Err(RmuxError::IncompleteFrame {
            expected: 4,
            received: frame.len(),
        });
    }

    let length = frame_length(frame)?;
    if length == 0 {
        return Err(RmuxError::EmptyFrame);
    }

    if length > DEFAULT_MAX_FRAME_LENGTH {
        return Err(RmuxError::FrameTooLarge {
            length,
            maximum: DEFAULT_MAX_FRAME_LENGTH,
        });
    }

    let required = 4 + length;
    if frame.len() < required {
        return Err(RmuxError::IncompleteFrame {
            expected: length,
            received: frame.len() - 4,
        });
    }

    if frame.len() > required {
        return Err(RmuxError::Decode(
            "trailing bytes remain after the first frame".to_owned(),
        ));
    }

    decode_payload(&frame[4..required])
}

/// Incremental detached frame decoder for partial socket reads.
#[derive(Debug, Clone)]
pub struct FrameDecoder {
    max_frame_length: usize,
    buffer: Vec<u8>,
}

impl FrameDecoder {
    /// Creates a decoder with the default maximum frame length.
    #[must_use]
    pub fn new() -> Self {
        Self::with_max_frame_length(DEFAULT_MAX_FRAME_LENGTH)
    }

    /// Creates a decoder with a custom maximum frame length.
    #[must_use]
    pub fn with_max_frame_length(max_frame_length: usize) -> Self {
        Self {
            max_frame_length,
            buffer: Vec::new(),
        }
    }

    /// Appends more raw transport bytes to the internal buffer.
    pub fn push_bytes(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// Attempts to decode the next complete frame from buffered bytes.
    pub fn next_frame<T>(&mut self) -> Result<Option<T>, RmuxError>
    where
        T: DeserializeOwned,
    {
        if self.buffer.len() < 4 {
            return Ok(None);
        }

        let length = frame_length(&self.buffer)?;
        if length == 0 {
            self.buffer.drain(..4);
            return Err(RmuxError::EmptyFrame);
        }

        if length > self.max_frame_length {
            self.buffer.clear();
            return Err(RmuxError::FrameTooLarge {
                length,
                maximum: self.max_frame_length,
            });
        }

        let required = 4 + length;
        if self.buffer.len() < required {
            return Ok(None);
        }

        let frame: Vec<u8> = self.buffer.drain(..required).collect();
        decode_payload(&frame[4..])
            .map(Some)
            .map_err(|error| match error {
                RmuxError::Decode(_) => {
                    self.buffer.clear();
                    error
                }
                _ => error,
            })
    }

    /// Returns any bytes remaining in the internal buffer after the last
    /// successfully decoded frame.
    #[must_use]
    pub fn remaining_bytes(&self) -> &[u8] {
        &self.buffer
    }
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

fn frame_length(buffer: &[u8]) -> Result<usize, RmuxError> {
    let header = buffer.get(..4).ok_or(RmuxError::IncompleteFrame {
        expected: 4,
        received: buffer.len(),
    })?;
    let header = <[u8; 4]>::try_from(header).map_err(|_| RmuxError::IncompleteFrame {
        expected: 4,
        received: buffer.len(),
    })?;

    Ok(u32::from_le_bytes(header) as usize)
}

fn decode_payload<T>(payload: &[u8]) -> Result<T, RmuxError>
where
    T: DeserializeOwned,
{
    bincode::deserialize(payload).map_err(|error| RmuxError::Decode(error.to_string()))
}

#[cfg(test)]
#[path = "codec/tests.rs"]
mod tests;
