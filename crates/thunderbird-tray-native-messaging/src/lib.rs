// SPDX-License-Identifier: GPL-3.0-only

//! Native Messaging length framing, independent of JSON protocol parsing.

use std::io::{self, Read, Write};

use thiserror::Error;

/// Maximum frame accepted or produced by the host: one mebibyte.
///
/// This matches the browser-facing limit for messages emitted by a native host
/// and deliberately applies the same bounded allocation policy to input.
pub const MAX_FRAME_SIZE: usize = 1024 * 1024;

const LENGTH_PREFIX_SIZE: usize = size_of::<u32>();

/// Native Messaging framing failure.
#[derive(Debug, Error)]
pub enum FramingError {
    /// The stream failed for a reason other than a clean or truncated EOF.
    #[error("native messaging I/O failed: {0}")]
    Io(#[from] io::Error),
    /// EOF arrived after only part of the four-byte length prefix.
    #[error("truncated native messaging length prefix: received {received} of 4 bytes")]
    TruncatedLengthPrefix {
        /// Number of prefix bytes received before EOF.
        received: usize,
    },
    /// EOF arrived before the declared payload length was satisfied.
    #[error("truncated native messaging frame: expected {expected} bytes, received {received}")]
    TruncatedFrame {
        /// Length declared by the prefix.
        expected: usize,
        /// Payload bytes received before EOF.
        received: usize,
    },
    /// The declared or supplied payload exceeds [`MAX_FRAME_SIZE`].
    #[error("native messaging frame is {size} bytes; maximum is {maximum}")]
    FrameTooLarge {
        /// Declared or supplied payload length.
        size: usize,
        /// Configured maximum frame length.
        maximum: usize,
    },
}

/// Reads one little-endian length-prefixed Native Messaging frame.
///
/// Returns `Ok(None)` only for a clean EOF before any prefix byte is read.
/// Frame size is validated before allocating its payload buffer.
pub fn read_frame(reader: &mut impl Read) -> Result<Option<Vec<u8>>, FramingError> {
    let mut prefix = [0_u8; LENGTH_PREFIX_SIZE];
    let prefix_read = read_until_full(reader, &mut prefix)?;

    if prefix_read == 0 {
        return Ok(None);
    }
    if prefix_read != prefix.len() {
        return Err(FramingError::TruncatedLengthPrefix {
            received: prefix_read,
        });
    }

    let size = u32::from_le_bytes(prefix) as usize;
    ensure_frame_size(size)?;

    let mut payload = vec![0_u8; size];
    let payload_read = read_until_full(reader, &mut payload)?;
    if payload_read != size {
        return Err(FramingError::TruncatedFrame {
            expected: size,
            received: payload_read,
        });
    }

    Ok(Some(payload))
}

/// Writes and flushes one little-endian length-prefixed Native Messaging frame.
pub fn write_frame(writer: &mut impl Write, payload: &[u8]) -> Result<(), FramingError> {
    ensure_frame_size(payload.len())?;
    let size = u32::try_from(payload.len()).map_err(|_| FramingError::FrameTooLarge {
        size: payload.len(),
        maximum: MAX_FRAME_SIZE,
    })?;

    writer.write_all(&size.to_le_bytes())?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(())
}

fn ensure_frame_size(size: usize) -> Result<(), FramingError> {
    if size > MAX_FRAME_SIZE {
        return Err(FramingError::FrameTooLarge {
            size,
            maximum: MAX_FRAME_SIZE,
        });
    }
    Ok(())
}

fn read_until_full(reader: &mut impl Read, buffer: &mut [u8]) -> Result<usize, io::Error> {
    let mut received = 0;
    while received < buffer.len() {
        match reader.read(&mut buffer[received..]) {
            Ok(0) => break,
            Ok(count) => received += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(received)
}
