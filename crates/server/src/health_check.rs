//! `binge-server health`: asks a running server whether it is well.
//!
//! This is what the Docker image's health check runs, so the image needs no
//! other program to ask with. It speaks just enough HTTP for the one
//! question, over a plain socket.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// How long connecting may take, and then each wait for the answer. Short,
/// because what asks gives up after a few seconds itself.
const PATIENCE: Duration = Duration::from_secs(2);

/// The most that is read of an answer. A health answer is a few hundred bytes.
const MOST_READ: u64 = 64 * 1024;

/// The part of `GET /api/v1/health` that says whether the server is well.
#[derive(Deserialize)]
struct Answer {
    version: String,
    database: String,
}

/// Where a server that listens on `listening` is reached from its own
/// machine: one that listens on every address is asked on the loopback one.
pub fn reached_at(listening: SocketAddr) -> SocketAddr {
    let ip = match listening.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => Ipv4Addr::LOCALHOST.into(),
        IpAddr::V6(ip) if ip.is_unspecified() => Ipv6Addr::LOCALHOST.into(),
        ip => ip,
    };
    SocketAddr::new(ip, listening.port())
}

/// Asks the server at `address` how it is. Answers with its version when it
/// is well, and with what is wrong when it is not or when nothing answers.
pub fn ask(address: SocketAddr) -> Result<String> {
    let mut stream = TcpStream::connect_timeout(&address, PATIENCE)
        .with_context(|| format!("no server answers at {address}"))?;
    stream.set_read_timeout(Some(PATIENCE))?;
    stream.set_write_timeout(Some(PATIENCE))?;

    // HTTP/1.0, so that the answer comes in one piece and the server closes
    // the connection after it. An IP address is a host the server always
    // answers to.
    write!(
        stream,
        "GET /api/v1/health HTTP/1.0\r\nHost: {address}\r\n\r\n"
    )
    .with_context(|| format!("could not ask the server at {address}"))?;
    let mut answer = Vec::new();
    stream
        .take(MOST_READ)
        .read_to_end(&mut answer)
        .with_context(|| format!("the server at {address} did not answer in time"))?;
    read(&answer).with_context(|| format!("the server at {address} is not well"))
}

/// The version in a whole HTTP answer to the question, when the answer says
/// the server is well.
fn read(answer: &[u8]) -> Result<String> {
    let end_of_head = answer
        .windows(4)
        .position(|bytes| bytes == b"\r\n\r\n")
        .context("what answered does not speak HTTP")?;
    let (head, body) = (&answer[..end_of_head], &answer[end_of_head + 4..]);

    // "HTTP/1.0 200 OK"
    let status = String::from_utf8_lossy(head)
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned();
    if status != "200" {
        bail!("it answered with status {status}");
    }
    let answer: Answer =
        serde_json::from_slice(body).context("what answered is not a Binge server")?;
    if answer.database != "ok" {
        bail!("its database does not answer");
    }
    Ok(answer.version)
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    /// Answers the next connection on this machine with `answer`, whatever
    /// is asked, and says where it listens.
    fn answering(answer: &'static str) -> SocketAddr {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
        let address = listener.local_addr().expect("the port it took");
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("a connection");
            // The question ends with an empty line.
            let mut asked = Vec::new();
            let mut byte = [0];
            while !asked.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
                asked.push(byte[0]);
            }
            let _ = stream.write_all(answer.as_bytes());
        });
        address
    }

    /// What `ask` says is wrong, with its causes, in one line.
    fn wrong(address: SocketAddr) -> String {
        format!("{:#}", ask(address).expect_err("the server is not well"))
    }

    #[test]
    fn a_server_that_is_well_says_its_version() {
        let address = answering(
            "HTTP/1.0 200 OK\r\ncontent-type: application/json\r\n\r\n\
             {\"name\":\"Binge Server\",\"version\":\"1.2.3\",\"database\":\"ok\",\"needs_setup\":true}",
        );
        assert_eq!(ask(address).expect("it is well"), "1.2.3");
    }

    #[test]
    fn a_server_without_its_database_is_not_well() {
        let address = answering(
            "HTTP/1.0 200 OK\r\n\r\n{\"version\":\"1.2.3\",\"database\":\"unavailable\"}",
        );
        assert!(wrong(address).ends_with("its database does not answer"));
    }

    #[test]
    fn an_answer_that_is_not_ok_is_not_well() {
        let address = answering("HTTP/1.0 403 Forbidden\r\n\r\n{}");
        assert!(wrong(address).ends_with("it answered with status 403"));
    }

    #[test]
    fn another_program_on_the_port_is_not_a_server() {
        let address = answering("HTTP/1.0 200 OK\r\n\r\n<html>hello</html>");
        assert!(wrong(address).contains("what answered is not a Binge server"));

        let address = answering("220 mail ready\r\n");
        assert!(wrong(address).ends_with("what answered does not speak HTTP"));
    }

    #[test]
    fn nothing_listening_is_said_plainly() {
        // A port that was free a moment ago, with nothing on it now.
        let address = {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
            listener.local_addr().expect("the port it took")
        };
        assert!(wrong(address).starts_with(&format!("no server answers at {address}")));
    }

    #[test]
    fn a_server_on_every_address_is_asked_on_this_machine() {
        let reached = |listening: &str| reached_at(listening.parse().expect("an address"));
        assert_eq!(reached("0.0.0.0:8686").to_string(), "127.0.0.1:8686");
        assert_eq!(reached("[::]:8686").to_string(), "[::1]:8686");
        assert_eq!(
            reached("192.168.1.20:9000").to_string(),
            "192.168.1.20:9000"
        );
    }
}
