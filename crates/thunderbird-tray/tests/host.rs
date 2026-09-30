// SPDX-License-Identifier: GPL-3.0-only

use std::io::Cursor;

use thunderbird_tray::run_host;
use thunderbird_tray_native_messaging::{read_frame, write_frame};
use thunderbird_tray_protocol::{
    DecodeOutcome, FullStatePayload, HelloAckPayload, HelloPayload, Message, decode, encode,
};

fn input_stream(messages: &[Message]) -> Vec<u8> {
    let mut stream = Vec::new();
    for message in messages {
        write_frame(&mut stream, &encode(message).unwrap()).unwrap();
    }
    stream
}

#[test]
fn fake_extension_completes_handshake_and_sends_initial_state() {
    let input = input_stream(&[
        Message::Hello(HelloPayload {
            extension_version: "0.1.0".into(),
            thunderbird_version: "156.0.1".into(),
        }),
        Message::FullState(FullStatePayload {
            total_unread: 3,
            accounts: Vec::new(),
        }),
    ]);
    let mut output = Vec::new();

    run_host(&mut Cursor::new(input), &mut output).unwrap();

    let mut output = Cursor::new(output);
    let response = read_frame(&mut output).unwrap().unwrap();
    assert_eq!(
        decode(&response).unwrap(),
        DecodeOutcome::Message(Message::HelloAck(HelloAckPayload {
            host_version: env!("CARGO_PKG_VERSION").into(),
        }))
    );
    assert_eq!(read_frame(&mut output).unwrap(), None);
}

#[test]
fn repeated_hello_is_safe_and_each_session_start_is_acknowledged() {
    let hello = Message::Hello(HelloPayload {
        extension_version: "0.1.0".into(),
        thunderbird_version: "156.0.1".into(),
    });
    let input = input_stream(&[hello.clone(), hello]);
    let mut output = Vec::new();

    run_host(&mut Cursor::new(input), &mut output).unwrap();

    let mut output = Cursor::new(output);
    assert!(read_frame(&mut output).unwrap().is_some());
    assert!(read_frame(&mut output).unwrap().is_some());
    assert_eq!(read_frame(&mut output).unwrap(), None);
}

#[test]
fn pre_handshake_state_and_unknown_messages_never_pollute_stdout() {
    let state = Message::FullState(FullStatePayload {
        total_unread: 9,
        accounts: Vec::new(),
    });
    let mut input = input_stream(&[state]);
    let unknown = br#"{"protocol":1,"type":"futureMessage","payload":{"secret":"x"}}"#;
    write_frame(&mut input, unknown).unwrap();
    let mut output = Vec::new();

    run_host(&mut Cursor::new(input), &mut output).unwrap();

    assert!(output.is_empty());
}
