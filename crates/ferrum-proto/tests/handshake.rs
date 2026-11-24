//! Version negotiation and the handshake (`docs/02` §4, `docs/06` §7, `docs/10` §2), end to end
//! through frames: the client's `Hello` is framed, split, decoded and answered.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a failed expectation is a failed test"
)]

use ferrum_proto::{
    ClientKind, ClientMessage, DaemonMessage, Hello, NONCE_LEN, Nonce, PROTOCOL_VERSION,
    ProtoError, ProtocolVersion, Refusal, RefusalReason, answer_hello, decode_message,
    encode_frame, split_frame,
};

const DAEMON_NONCE: Nonce = Nonce([0x5A; NONCE_LEN]);

fn hello(protocol_version: ProtocolVersion, nonce: Nonce) -> Hello {
    Hello { protocol_version, client_kind: ClientKind::Ui, nonce }
}

/// Sends `hello` through a frame and returns the daemon's answer.
fn connect(hello: Hello) -> DaemonMessage {
    let mut wire = Vec::new();
    encode_frame(&ClientMessage::Hello(hello), &mut wire).unwrap();
    let frame = split_frame(&wire).unwrap().unwrap();
    let ClientMessage::Hello(received) = decode_message(frame.payload).unwrap() else {
        panic!("not a hello");
    };
    answer_hello(&received, &DAEMON_NONCE)
}

const fn version(major: u16, minor: u16) -> ProtocolVersion {
    ProtocolVersion { major, minor }
}

fn refusal(reason: RefusalReason) -> DaemonMessage {
    DaemonMessage::Refused(Refusal { daemon_version: PROTOCOL_VERSION, reason })
}

#[test]
fn the_same_version_is_welcomed() {
    let answer = connect(hello(PROTOCOL_VERSION, DAEMON_NONCE));
    assert_eq!(answer, DaemonMessage::Welcome { protocol_version: PROTOCOL_VERSION });
}

#[test]
fn a_newer_minor_client_talks_at_the_daemon_minor() {
    let newer = version(PROTOCOL_VERSION.major, PROTOCOL_VERSION.minor + 1);
    let answer = connect(hello(newer, DAEMON_NONCE));
    assert_eq!(answer, DaemonMessage::Welcome { protocol_version: PROTOCOL_VERSION });
}

#[test]
fn an_older_minor_is_negotiated_down() {
    let ours = version(1, 4);
    assert_eq!(ours.negotiate(version(1, 2)).unwrap(), version(1, 2));
    assert_eq!(version(1, 2).negotiate(ours).unwrap(), version(1, 2));
}

#[test]
fn another_major_is_refused_with_both_versions() {
    for major in [PROTOCOL_VERSION.major - 1, PROTOCOL_VERSION.major + 1] {
        let theirs = version(major, 0);
        assert_eq!(connect(hello(theirs, DAEMON_NONCE)), refusal(RefusalReason::VersionMismatch));
        let error = PROTOCOL_VERSION.negotiate(theirs).unwrap_err();
        assert!(
            matches!(error, ProtoError::VersionMismatch { ours, theirs: t } if ours == PROTOCOL_VERSION && t == theirs)
        );
        assert_eq!(
            error.to_string(),
            format!("protocol {theirs} is incompatible with protocol {PROTOCOL_VERSION}")
        );
    }
}

#[test]
fn a_wrong_nonce_is_refused_before_the_version_is_looked_at() {
    let mut wrong = DAEMON_NONCE;
    wrong.0[NONCE_LEN - 1] ^= 1;
    assert_eq!(connect(hello(PROTOCOL_VERSION, wrong.clone())), refusal(RefusalReason::BadNonce));
    assert_eq!(connect(hello(version(9, 0), wrong)), refusal(RefusalReason::BadNonce));
}

#[test]
fn the_nonce_never_appears_in_debug_output() {
    let text = format!("{:?}", hello(PROTOCOL_VERSION, DAEMON_NONCE));
    assert!(!text.contains("90"), "{text}");
    assert!(text.contains("redacted"));
}

#[test]
fn nonce_equality_is_the_constant_time_match() {
    let mut other = DAEMON_NONCE;
    assert_eq!(other, DAEMON_NONCE);
    other.0[0] ^= 0x80;
    assert_ne!(other, DAEMON_NONCE);
    assert_eq!(other == DAEMON_NONCE, other.matches(&DAEMON_NONCE));
}

/// The handshake keeps its bytes in every major version (see the `handshake` module docs): a
/// `Hello` from major 7 and the daemon's answers are still these exact frames.
#[test]
fn handshake_frames_are_pinned_for_every_major() {
    let future = version(7, 3);
    let mut wire = Vec::new();
    encode_frame(&ClientMessage::Hello(hello(future, Nonce([0x11; NONCE_LEN]))), &mut wire)
        .unwrap();
    let mut expected = vec![0x24, 0, 0, 0, 0x00, 0x07, 0x03, 0x00];
    expected.extend([0x11; NONCE_LEN]);
    assert_eq!(wire, expected, "Hello: variant 0, major, minor, client kind, nonce");

    let answers = [
        (DaemonMessage::Welcome { protocol_version: future }, vec![3, 0, 0, 0, 0x00, 0x07, 0x03]),
        (
            DaemonMessage::Refused(Refusal {
                daemon_version: future,
                reason: RefusalReason::VersionMismatch,
            }),
            vec![4, 0, 0, 0, 0x01, 0x07, 0x03, 0x00],
        ),
        (
            DaemonMessage::Refused(Refusal {
                daemon_version: future,
                reason: RefusalReason::BadNonce,
            }),
            vec![4, 0, 0, 0, 0x01, 0x07, 0x03, 0x01],
        ),
    ];
    for (answer, bytes) in answers {
        let mut wire = Vec::new();
        encode_frame(&answer, &mut wire).unwrap();
        assert_eq!(wire, bytes, "{answer:?}");
        let frame = split_frame(&wire).unwrap().unwrap();
        assert_eq!(decode_message::<DaemonMessage>(frame.payload).unwrap(), answer);
    }
}
