// SPDX-License-Identifier: GPL-3.0-only

use std::io::{self, Cursor, Read};

use thunderbird_tray_native_messaging::{FramingError, MAX_FRAME_SIZE, read_frame, write_frame};

struct ChunkedReader<R> {
    inner: R,
    chunk_size: usize,
}

impl<R: Read> Read for ChunkedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let limit = buffer.len().min(self.chunk_size);
        self.inner.read(&mut buffer[..limit])
    }
}

fn framed(payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_frame(&mut bytes, payload).unwrap();
    bytes
}

#[test]
fn clean_eof_is_distinct_from_a_truncated_frame() {
    assert_eq!(read_frame(&mut Cursor::new([])).unwrap(), None);
}

#[test]
fn partial_prefix_and_body_reads_are_reassembled() {
    let payload = br#"{"protocol":1}"#;
    let mut reader = ChunkedReader {
        inner: Cursor::new(framed(payload)),
        chunk_size: 1,
    };

    assert_eq!(read_frame(&mut reader).unwrap(), Some(payload.to_vec()));
}

#[test]
fn multiple_frames_are_read_without_losing_boundaries() {
    let mut bytes = framed(b"first");
    bytes.extend(framed(b"second"));
    let mut reader = Cursor::new(bytes);

    assert_eq!(read_frame(&mut reader).unwrap(), Some(b"first".to_vec()));
    assert_eq!(read_frame(&mut reader).unwrap(), Some(b"second".to_vec()));
    assert_eq!(read_frame(&mut reader).unwrap(), None);
}

#[test]
fn truncated_length_prefix_is_rejected() {
    let error = read_frame(&mut Cursor::new([3_u8, 0])).unwrap_err();
    assert!(matches!(
        error,
        FramingError::TruncatedLengthPrefix { received: 2 }
    ));
}

#[test]
fn truncated_payload_is_rejected() {
    let mut bytes = 5_u32.to_le_bytes().to_vec();
    bytes.extend(b"abc");

    let error = read_frame(&mut Cursor::new(bytes)).unwrap_err();
    assert!(matches!(
        error,
        FramingError::TruncatedFrame {
            expected: 5,
            received: 3,
        }
    ));
}

#[test]
fn oversized_declared_frame_is_rejected_before_payload_allocation() {
    let declared = u32::try_from(MAX_FRAME_SIZE + 1).unwrap();
    let mut reader = Cursor::new(declared.to_le_bytes());

    let error = read_frame(&mut reader).unwrap_err();
    assert!(matches!(
        error,
        FramingError::FrameTooLarge {
            size,
            maximum: MAX_FRAME_SIZE,
        } if size == MAX_FRAME_SIZE + 1
    ));
    assert_eq!(reader.position(), 4);
}

#[test]
fn oversized_outgoing_frame_is_rejected_without_writing() {
    let payload = vec![0_u8; MAX_FRAME_SIZE + 1];
    let mut output = Vec::new();

    assert!(matches!(
        write_frame(&mut output, &payload),
        Err(FramingError::FrameTooLarge {
            size,
            maximum: MAX_FRAME_SIZE,
        }) if size == MAX_FRAME_SIZE + 1
    ));
    assert!(output.is_empty());
}

#[test]
fn empty_payload_has_a_valid_zero_length_frame() {
    let bytes = framed(&[]);
    assert_eq!(bytes, [0, 0, 0, 0]);
    assert_eq!(
        read_frame(&mut Cursor::new(bytes)).unwrap(),
        Some(Vec::new())
    );
}
