// SPDX-License-Identifier: GPL-3.0-only

use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use thunderbird_tray_protocol::{
    AccountState, DecodeError, DecodeOutcome, FullStatePayload, HelloAckPayload, HelloPayload,
    Message, PROTOCOL_VERSION, RequestFullStatePayload, decode, encode,
};

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/protocol/v1")
        .join(name);
    fs::read(path).unwrap()
}

fn cases() -> [(&'static str, Message); 4] {
    [
        (
            "hello.json",
            Message::Hello(HelloPayload {
                extension_version: "0.1.0".into(),
                thunderbird_version: "156.0.1".into(),
            }),
        ),
        (
            "hello-ack.json",
            Message::HelloAck(HelloAckPayload {
                host_version: "0.1.0".into(),
            }),
        ),
        (
            "full-state.json",
            Message::FullState(FullStatePayload {
                total_unread: 12,
                accounts: vec![
                    AccountState {
                        id: "account1".into(),
                        name: "Personal".into(),
                        unread: 7,
                    },
                    AccountState {
                        id: "account2".into(),
                        name: "Work".into(),
                        unread: 5,
                    },
                ],
            }),
        ),
        (
            "request-full-state.json",
            Message::RequestFullState(RequestFullStatePayload {}),
        ),
    ]
}

#[test]
fn golden_fixtures_decode_to_typed_messages() {
    for (name, expected) in cases() {
        assert_eq!(
            decode(&fixture(name)).unwrap(),
            DecodeOutcome::Message(expected)
        );
    }
}

#[test]
fn typed_messages_serialize_to_golden_fixtures() {
    for (name, message) in cases() {
        let actual: Value = serde_json::from_slice(&encode(&message).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(&fixture(name)).unwrap();
        assert_eq!(actual, expected, "fixture {name}");
    }
}

#[test]
fn every_initial_message_round_trips() {
    for (_, message) in cases() {
        let encoded = encode(&message).unwrap();
        assert_eq!(decode(&encoded).unwrap(), DecodeOutcome::Message(message));
    }
}

#[test]
fn unknown_message_types_are_recoverable_and_do_not_retain_payloads() {
    let input = br#"{
        "protocol": 1,
        "type": "futureMessage",
        "payload": {"subject": "must not be retained"}
    }"#;

    assert_eq!(
        decode(input).unwrap(),
        DecodeOutcome::UnknownMessageType {
            protocol: PROTOCOL_VERSION,
            message_type: "futureMessage".into(),
        }
    );
}

#[test]
fn unsupported_protocol_versions_fail_clearly() {
    let input = br#"{"protocol":2,"type":"requestFullState","payload":{}}"#;

    assert!(matches!(
        decode(input),
        Err(DecodeError::UnsupportedProtocolVersion {
            received: 2,
            supported: PROTOCOL_VERSION,
        })
    ));
}

#[test]
fn malformed_json_and_malformed_known_payloads_are_rejected() {
    assert!(matches!(
        decode(br#"{"protocol":1"#),
        Err(DecodeError::InvalidJson(_))
    ));
    assert!(matches!(
        decode(br#"{"protocol":1,"type":"hello","payload":{}}"#),
        Err(DecodeError::InvalidJson(_))
    ));
}
