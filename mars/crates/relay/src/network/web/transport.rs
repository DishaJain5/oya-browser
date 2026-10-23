/** Validated exact-address connections preserve hostname verification without DNS rebinding. */
use crate::constants::{DNS_ANSWERS, NETWORK_CHUNK_BYTES};
use crate::network::policy::{denied, public_address};
use crate::network::{Method, WebRequest};
use crate::protocol::{EngineError, EngineResult};
use ::url::{Host, Url};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

/** TLS uses the same owned HTTP codec and socket deadline as plaintext. */
pub(super) enum Connection {
    /** Exact policy-validated OS socket. */
    Plain(TcpStream),
    /** Rustls performs normal chain, hostname and signature validation. */
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
}

/** Resolve once, reject every nonpublic answer, then connect to one validated address. */
pub(super) fn connect(url: &Url, fixture: bool, deadline: Instant) -> EngineResult<Connection> {
    let addresses = resolve(url, fixture)?;
    let mut last = network_error();
    for address in addresses {
        match TcpStream::connect_timeout(&address, remaining(deadline)?) {
            Ok(socket) => {
                let mut connection = wrap(socket, url)?;
                connection.handshake(deadline)?;
                return Ok(connection);
            }
            Err(error) => last = io_error(error),
        }
    }
    Err(last)
}

/** Standard DNS remains inside the supervised process so a stalled resolver can be killed. */
fn resolve(url: &Url, fixture: bool) -> EngineResult<Vec<SocketAddr>> {
    let port = url.port_or_known_default().ok_or_else(denied)?;
    let host = url.host().ok_or_else(denied)?;
    let addresses: Vec<SocketAddr> = match host {
        Host::Ipv4(ip) => vec![SocketAddr::new(IpAddr::V4(ip), port)],
        Host::Ipv6(ip) => vec![SocketAddr::new(IpAddr::V6(ip), port)],
        Host::Domain(host) => (host, port)
            .to_socket_addrs()
            .map_err(io_error)?
            .take(DNS_ANSWERS + 1)
            .collect(),
    };
    if addresses.is_empty()
        || addresses.len() > DNS_ANSWERS
        || addresses.iter().any(|address| {
            !(public_address(address.ip()) || (fixture && address.ip().is_loopback()))
        })
    {
        return Err(denied());
    }
    Ok(addresses)
}

/** SNI and certificate verification use the URL hostname, never the resolved IP substitute. */
fn wrap(socket: TcpStream, url: &Url) -> EngineResult<Connection> {
    if url.scheme() == "http" {
        return Ok(Connection::Plain(socket));
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| tls_error())?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(
        url.host_str()
            .ok_or_else(denied)?
            .trim_matches(['[', ']'])
            .to_owned(),
    )
    .map_err(|_| tls_error())?;
    let connection = ClientConnection::new(Arc::new(config), name).map_err(|_| tls_error())?;
    Ok(Connection::Tls(Box::new(StreamOwned::new(
        connection, socket,
    ))))
}

impl Connection {
    /** Prepared socket lifetime is bounded even before a transmit operation arrives. */
    pub(super) fn lease(&self, deadline: Instant) -> EngineResult<super::lease::Lease> {
        super::lease::Lease::start(
            match self {
                Self::Plain(socket) => socket,
                Self::Tls(stream) => &stream.sock,
            },
            deadline,
        )
    }
    /** HTTP framing is constructed from normalized URL fields, without ambient proxy state. */
    pub(super) fn write_request(&mut self, url: &Url, deadline: Instant) -> EngineResult<()> {
        self.write_web_request(url, &WebRequest::get(url.as_str()), deadline)
    }
    /** No application bytes leave until the coordinator authorizes transmission. */
    pub(super) fn write_web_request(
        &mut self,
        url: &Url,
        request: &WebRequest,
        deadline: Instant,
    ) -> EngineResult<()> {
        let host = url.host_str().ok_or_else(denied)?;
        let host = url
            .port()
            .map(|port| format!("{host}:{port}"))
            .unwrap_or_else(|| host.to_owned());
        let target = format!(
            "{}{}",
            url.path(),
            url.query()
                .map(|query| format!("?{query}"))
                .unwrap_or_default()
        );
        let method = if request.method == Method::Post {
            "POST"
        } else {
            "GET"
        };
        let cookie = if request.cookie.is_empty() {
            String::new()
        } else {
            format!("Cookie: {}\r\n", request.cookie)
        };
        let content = if request.method == Method::Post {
            format!(
                "Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n",
                request.body.len()
            )
        } else {
            String::new()
        };
        let wire = format!(
            "{method} {target} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nAccept: text/html, text/css\r\nAccept-Encoding: identity\r\nUser-Agent: Oya-Native/0.3\r\n{cookie}{content}\r\n{}",
            request.body
        );
        let mut bytes = wire.as_bytes();
        while !bytes.is_empty() {
            self.deadline(deadline)?;
            let count = match self {
                Self::Plain(socket) => socket.write(bytes),
                Self::Tls(stream) => stream.write(bytes),
            }
            .map_err(io_error)?;
            if count == 0 {
                return Err(network_error());
            }
            bytes = &bytes[count..];
        }
        Ok(())
    }

    /** TLS negotiation finishes in preparation, while stale ownership can still prevent POST. */
    fn handshake(&mut self, deadline: Instant) -> EngineResult<()> {
        if let Self::Tls(stream) = self {
            while stream.conn.is_handshaking() {
                let remaining = remaining(deadline)?;
                stream
                    .sock
                    .set_read_timeout(Some(remaining))
                    .map_err(io_error)?;
                stream
                    .sock
                    .set_write_timeout(Some(remaining))
                    .map_err(io_error)?;
                stream
                    .conn
                    .complete_io(&mut stream.sock)
                    .map_err(io_error)?;
            }
        }
        Ok(())
    }

    /** Reads and TLS handshake I/O share the remaining absolute request budget. */
    pub(super) fn read(&mut self, bytes: &mut [u8], deadline: Instant) -> EngineResult<usize> {
        self.deadline(deadline)?;
        match self {
            Self::Plain(socket) => socket.read(bytes),
            Self::Tls(stream) => stream.read(bytes),
        }
        .map_err(io_error)
    }

    /** Every socket operation receives both timeouts because TLS reads may also write. */
    fn deadline(&self, deadline: Instant) -> EngineResult<()> {
        let socket = match self {
            Self::Plain(socket) => socket,
            Self::Tls(stream) => &stream.sock,
        };
        let duration = remaining(deadline)?;
        socket.set_read_timeout(Some(duration)).map_err(io_error)?;
        socket.set_write_timeout(Some(duration)).map_err(io_error)
    }
}

/** One total budget covers redirects, handshake and body reads. */
fn remaining(deadline: Instant) -> EngineResult<Duration> {
    let duration = deadline.saturating_duration_since(Instant::now());
    if duration.is_zero() {
        return Err(EngineError::new(
            "timeout",
            "The web request exceeded its eight-second deadline.",
            "Retry the destination.",
        ));
    }
    Ok(duration)
}

/** Rustls invalid-data errors expose no certificate bypass. */
fn io_error(error: std::io::Error) -> EngineError {
    if error.kind() == std::io::ErrorKind::InvalidData {
        return tls_error();
    }
    if [std::io::ErrorKind::TimedOut, std::io::ErrorKind::WouldBlock].contains(&error.kind()) {
        return EngineError::new(
            "timeout",
            "The web request timed out.",
            "Retry the destination.",
        );
    }
    network_error()
}

/** Native TLS copy gives the user an explicit final refusal. */
fn tls_error() -> EngineError {
    EngineError::new(
        "tls_error",
        "The server's certificate could not be verified.",
        "Check the destination and its certificate; verification cannot be bypassed.",
    )
}
/** Errors retain the committed page and permit a later explicit retry. */
fn network_error() -> EngineError {
    EngineError::new(
        "network_error",
        "The destination could not be reached or returned an incomplete response.",
        "Check the URL and retry.",
    )
}

/** Fixed read chunks keep framing allocation bounded independently of socket buffering. */
pub(super) const READ_CHUNK: usize = NETWORK_CHUNK_BYTES;
