// SPDX-License-Identifier: GPL-3.0-only

use std::io::{Cursor, Write};
use std::process::{Command, Stdio};

use thunderbird_tray_native_messaging::{read_frame, write_frame};
use thunderbird_tray_protocol::{
    AccountState, DecodeOutcome, FullStatePayload, HelloPayload, Message, decode, encode,
};

#[test]
fn thunderbird_arguments_start_protocol_mode_with_clean_stdout() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_thunderbird-tray"))
        .args([
            "/tmp/io.github.apophuy.thunderbird_tray.json",
            "test-extension@example.invalid",
        ])
        .env(
            "XDG_CONFIG_HOME",
            std::env::temp_dir().join("thunderbird-tray-missing-test-config"),
        )
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/tmp/thunderbird-tray-missing-test-bus",
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut input = Vec::new();
    for message in [
        Message::Hello(HelloPayload {
            extension_version: "0.1.0".to_owned(),
            thunderbird_version: "156.0.1".to_owned(),
        }),
        Message::FullState(FullStatePayload {
            total_unread: 2,
            accounts: vec![AccountState {
                id: "account1".to_owned(),
                name: "Personal".to_owned(),
                unread: 2,
            }],
        }),
    ] {
        write_frame(&mut input, &encode(&message).unwrap()).unwrap();
    }
    child.stdin.take().unwrap().write_all(&input).unwrap();

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let mut stdout = Cursor::new(output.stdout);
    let response = read_frame(&mut stdout).unwrap().unwrap();
    assert!(matches!(
        decode(&response).unwrap(),
        DecodeOutcome::Message(Message::HelloAck(_))
    ));
    assert_eq!(read_frame(&mut stdout).unwrap(), None);

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("native messaging handshake completed"));
    assert!(stderr.contains("received Thunderbird Inbox unread state"));
}
