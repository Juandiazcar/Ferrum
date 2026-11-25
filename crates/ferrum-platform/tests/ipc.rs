//! The IPC endpoint on the running OS: a client of the same user connects, both directions carry
//! bytes (the client reading and writing on two threads at once), a second endpoint for the same run directory cannot steal the name of a live one
//! (Windows `FILE_FLAG_FIRST_PIPE_INSTANCE`; Unix: the instance lock guards it), and the socket
//! file is owner-only (Unix).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failure is a failed test"
)]

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use ferrum_platform::ipc::{self, Connection};
use mio::{Events, Interest, Poll, Token};

const LISTENER: Token = Token(0);
const CLIENT: Token = Token(1);
const DEADLINE: Duration = Duration::from_secs(10);

fn run_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ferrum-ipc-{}", std::process::id()));
    let _stale = std::fs::remove_dir_all(&dir);
    ferrum_platform::prepare_private_dir(&dir).unwrap();
    dir
}

/// Polls until the listener hands out a connection.
fn accept(poll: &mut Poll, listener: &mut dyn ipc::Listener) -> Box<dyn Connection> {
    let mut events = Events::with_capacity(8);
    let start = Instant::now();
    loop {
        if let Some(connection) = listener.accept().unwrap() {
            return connection;
        }
        assert!(start.elapsed() < DEADLINE, "no connection arrived");
        poll.poll(&mut events, Some(DEADLINE)).unwrap();
    }
}

/// Polls until `want` bytes were read from the connection.
fn read_exact(poll: &mut Poll, connection: &mut dyn Connection, want: usize) -> Vec<u8> {
    let mut events = Events::with_capacity(8);
    let mut got = Vec::new();
    let mut buf = [0; 64];
    let start = Instant::now();
    while got.len() < want {
        match connection.read(&mut buf) {
            Ok(0) => panic!("the client closed early"),
            Ok(n) => got.extend_from_slice(&buf[..n]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(start.elapsed() < DEADLINE, "the bytes never arrived");
                poll.poll(&mut events, Some(DEADLINE)).unwrap();
            }
            Err(error) => panic!("read failed: {error}"),
        }
    }
    got
}

#[test]
fn a_client_of_the_same_user_connects_and_talks() {
    let dir = run_dir();
    let mut poll = Poll::new().unwrap();
    let mut listener = ipc::listen(&dir).unwrap();
    poll.registry().register(&mut listener, LISTENER, Interest::READABLE).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(dir.join(ipc::SOCKET_FILE)).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    }
    #[cfg(windows)]
    assert!(ipc::listen(&dir).is_err(), "a second endpoint must not share the pipe name");
    let client = ipc::connect(&dir).unwrap();
    let (mut reader, mut writer) = (client.reader, client.writer);
    // The client reads on one thread while it writes on another, as the UI's IPC thread does: a
    // blocked read must not hold up the write it waits for an answer to.
    let answer = std::thread::spawn(move || {
        let mut reply = [0; 4];
        reader.read_exact(&mut reply).map(|()| reply)
    });
    let mut connection = accept(&mut poll, listener.as_mut());
    poll.registry()
        .register(&mut connection, CLIENT, Interest::READABLE | Interest::WRITABLE)
        .unwrap();
    writer.write_all(b"hello").unwrap();
    assert_eq!(read_exact(&mut poll, connection.as_mut(), 5), b"hello");
    assert!(connection.is_peer_owner().unwrap(), "the peer runs as this user");
    connection.write_all(b"back").unwrap();
    assert_eq!(&answer.join().unwrap().unwrap(), b"back");
    drop(connection);
    drop(listener);
    std::fs::remove_dir_all(&dir).unwrap();
}
