//! Source: original
//! Asking a running viewer over HTTP/1.1 on 127.0.0.1, with the standard library alone: the command line asks it what
//! it watches and asks it to stop. One request a connection, closed by the answer.

use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

/// How long to wait to connect: on this machine, a server that listens answers at once.
const CONNECT: Duration = Duration::from_secs(1);
/// How long to wait for an answer.
const ANSWER: Duration = Duration::from_secs(10);

/// What a server answered: its status and its body.
#[derive(Debug, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub body: Vec<u8>,
}

/// The address of `port` on this machine.
fn local(port: u16) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, port))
}

/// Whether anything listens on `port` of this machine.
pub fn listens(port: u16) -> bool {
    TcpStream::connect_timeout(&local(port), CONNECT).is_ok()
}

/// What the server on `port` answers `method` at `path`, or `None` if nothing there answers HTTP.
pub fn ask(port: u16, method: &str, path: &str) -> Option<Answer> {
    let mut stream = TcpStream::connect_timeout(&local(port), CONNECT).ok()?;
    stream.set_read_timeout(Some(ANSWER)).ok()?;
    stream.set_write_timeout(Some(ANSWER)).ok()?;
    let request =
        format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).ok()?;
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer).ok()?;
    parse(&answer)
}

/// An HTTP/1.1 answer whose body runs to the end of the connection, as one that closes it gives.
fn parse(answer: &[u8]) -> Option<Answer> {
    let end = answer.windows(4).position(|window| window == b"\r\n\r\n")?;
    let head = std::str::from_utf8(answer.get(..end)?).ok()?;
    let status = head.strip_prefix("HTTP/1.1 ")?.get(..3)?.parse().ok()?;
    Some(Answer { status, body: answer.get(end.checked_add(4)?..)?.to_vec() })
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;

    #[test]
    fn an_answer_is_its_status_and_the_body_after_the_head() {
        let answer = parse(b"HTTP/1.1 403 Forbidden\r\ncontent-length: 2\r\n\r\nno").unwrap();
        assert_eq!(answer, Answer { status: 403, body: b"no".to_vec() });
        assert_eq!(parse(b"HTTP/1.1 200 OK\r\n"), None, "no end to the head");
        assert_eq!(parse(b"SSH-2.0\r\n\r\n"), None, "not HTTP");
    }

    #[test]
    fn a_port_nothing_listens_on_answers_nothing() {
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap().local_addr().unwrap().port();
        assert!(!listens(port));
        assert_eq!(ask(port, "GET", "/"), None);
    }
}
