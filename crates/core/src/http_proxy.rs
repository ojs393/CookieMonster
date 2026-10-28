
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, ServerConnection, StreamOwned};

use crate::ca::LocalCa;
use crate::engine::Engine;
use crate::types::{
    CookieAction, CookieAttributes, CookieEvent, CookieParty, DecisionKind,
};

pub const DEFAULT_PROXY_ADDR: &str = "127.0.0.1:8877";

#[derive(Clone)]
pub struct ProxyHandle {
    pub enabled: Arc<AtomicBool>,
}

impl ProxyHandle {
    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::SeqCst);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }
}

pub fn spawn(
    addr: &str,
    engine: Arc<Mutex<Engine>>,
    ca: Option<Arc<LocalCa>>,
    initially_on: bool,
) -> std::io::Result<ProxyHandle> {
    let listener = TcpListener::bind(addr)?;
    let enabled = Arc::new(AtomicBool::new(initially_on));
    let flag = Arc::clone(&enabled);
    let eng = Arc::clone(&engine);
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if !flag.load(Ordering::SeqCst) {
                let _ = stream.shutdown(std::net::Shutdown::Both);
                continue;
            }
            let eng = Arc::clone(&eng);
            let flag = Arc::clone(&flag);
            let ca = ca.clone();
            thread::spawn(move || {
                if flag.load(Ordering::SeqCst) {
                    let _ = handle_client(stream, eng, ca);
                }
            });
        }
    });
    Ok(ProxyHandle { enabled })
}

fn handle_client(
    client: TcpStream,
    engine: Arc<Mutex<Engine>>,
    ca: Option<Arc<LocalCa>>,
) -> std::io::Result<()> {
    client.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
    client.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;
    let mut reader = BufReader::new(client.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let parts: Vec<&str> = request_line.trim_end().split_whitespace().collect();
    if parts.len() < 2 {
        return Ok(());
    }
    let method = parts[0];
    let target = parts[1].to_string();

    let mut headers = Vec::<(String, String)>::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line == "\r\n" || line == "\n" || line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }

    if method.eq_ignore_ascii_case("CONNECT") {
        let mut client = reader.into_inner();
        client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")?;
        if let Some(ca) = ca {
            return mitm_https(client, &target, engine, ca);
        }
        return blind_tunnel(client, &target);
    }

    proxy_http(reader.into_inner(), method, &target, headers, engine)
}

fn blind_tunnel(mut client: TcpStream, target: &str) -> std::io::Result<()> {
    let mut upstream = TcpStream::connect(target)?;
    let mut c2 = client.try_clone()?;
    let mut u2 = upstream.try_clone()?;
    thread::spawn(move || {
        let _ = std::io::copy(&mut c2, &mut upstream);
    });
    let _ = std::io::copy(&mut u2, &mut client);
    Ok(())
}

fn mitm_https(
    client: TcpStream,
    target: &str,
    engine: Arc<Mutex<Engine>>,
    ca: Arc<LocalCa>,
) -> std::io::Result<()> {
    let host = target.split(':').next().unwrap_or(target).to_string();
    let server_cfg = ca
        .server_config_for_host(&host)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let server_conn = ServerConnection::new(server_cfg)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut client_tls: StreamOwned<ServerConnection, TcpStream> =
        StreamOwned::new(server_conn, client);

    let mut root = RootCertStore::empty();
    root.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let client_cfg = Arc::new(
        ClientConfig::builder()
            .with_root_certificates(root)
            .with_no_client_auth(),
    );
    let server_name = ServerName::try_from(host.clone())
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let upstream_tcp = TcpStream::connect(target)?;
    upstream_tcp.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
    upstream_tcp.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;
    let upstream_conn = ClientConnection::new(client_cfg, server_name)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut upstream_tls: StreamOwned<ClientConnection, TcpStream> =
        StreamOwned::new(upstream_conn, upstream_tcp);

    for _ in 0..64 {
        let req = match read_http_message(&mut client_tls) {
            Ok(Some(m)) => m,
            Ok(None) => break,
            Err(_) => break,
        };

        if let Some(cookie_header) = header_value(&req.headers, "cookie") {
            for name in cookie_names(cookie_header) {
                report(&engine, CookieAction::Send, &host, &name, "https");
            }
        }

        write_http_message(&mut upstream_tls, &req)?;

        let mut resp = match read_http_message(&mut upstream_tls) {
            Ok(Some(m)) => m,
            Ok(None) => break,
            Err(_) => break,
        };

        resp.headers.retain(|(k, v)| {
            if !k.eq_ignore_ascii_case("set-cookie") {
                return true;
            }
            let name = v.split('=').next().unwrap_or("").trim();
            if name.is_empty() {
                return true;
            }
            let decision = report(&engine, CookieAction::Set, &host, name, "https");
            !matches!(decision, DecisionKind::Deny | DecisionKind::Strip)
        });

        write_http_message(&mut client_tls, &resp)?;
    }
    Ok(())
}

struct HttpMessage {
    start_line: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn read_http_message(stream: &mut impl Read) -> std::io::Result<Option<HttpMessage>> {
    let mut reader = BufReader::new(stream);
    let mut start_line = String::new();
    if reader.read_line(&mut start_line)? == 0 {
        return Ok(None);
    }
    if start_line.trim().is_empty() {
        return Ok(None);
    }

    let mut headers = Vec::<(String, String)>::new();
    let mut content_length = 0usize;
    let mut chunked = false;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line == "\r\n" || line == "\n" || line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_string();
            let val = v.trim().to_string();
            if key.eq_ignore_ascii_case("content-length") {
                content_length = val.parse().unwrap_or(0);
            }
            if key.eq_ignore_ascii_case("transfer-encoding")
                && val.to_lowercase().contains("chunked")
            {
                chunked = true;
            }
            headers.push((key, val));
        }
    }

    let body = if chunked {
        read_chunked(&mut reader)?
    } else if content_length > 0 {
        let len = content_length.min(16 * 1024 * 1024);
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;
        buf
    } else {
        Vec::new()
    };

    Ok(Some(HttpMessage {
        start_line,
        headers,
        body,
    }))
}

fn write_http_message(stream: &mut impl Write, msg: &HttpMessage) -> std::io::Result<()> {
    stream.write_all(msg.start_line.as_bytes())?;
    for (k, v) in &msg.headers {
        if k.eq_ignore_ascii_case("content-length")
            || k.eq_ignore_ascii_case("transfer-encoding")
        {
            continue;
        }
        write!(stream, "{k}: {v}\r\n")?;
    }
    write!(stream, "Content-Length: {}\r\n", msg.body.len())?;
    stream.write_all(b"\r\n")?;
    if !msg.body.is_empty() {
        stream.write_all(&msg.body)?;
    }
    stream.flush()?;
    Ok(())
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn read_chunked(reader: &mut impl BufRead) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let mut size_line = String::new();
        reader.read_line(&mut size_line)?;
        let size = usize::from_str_radix(size_line.trim(), 16).unwrap_or(0);
        if size == 0 {
            let mut trailer = String::new();
            let _ = reader.read_line(&mut trailer)?;
            break;
        }
        let mut chunk = vec![0u8; size];
        reader.read_exact(&mut chunk)?;
        out.extend_from_slice(&chunk);
        let mut crlf = [0u8; 2];
        reader.read_exact(&mut crlf)?;
    }
    Ok(out)
}

fn proxy_http(
    mut client: TcpStream,
    method: &str,
    target: &str,
    headers: Vec<(String, String)>,
    engine: Arc<Mutex<Engine>>,
) -> std::io::Result<()> {
    let url = target.to_string();
    let (host, path) = match url.strip_prefix("http://") {
        Some(rest) => {
            if let Some((h, p)) = rest.split_once('/') {
                (h.to_string(), format!("/{p}"))
            } else {
                (rest.to_string(), "/".into())
            }
        }
        None => return Ok(()),
    };
    let host_only = host.split(':').next().unwrap_or(&host).to_string();

    if let Some((_, cookie_header)) = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("cookie"))
    {
        for name in cookie_names(cookie_header) {
            report(&engine, CookieAction::Send, &host_only, &name, "http");
        }
    }

    let mut upstream = TcpStream::connect(&host)?;
    write!(upstream, "{method} {path} HTTP/1.1\r\n")?;
    for (k, v) in &headers {
        if k.eq_ignore_ascii_case("proxy-connection") {
            continue;
        }
        write!(upstream, "{k}: {v}\r\n")?;
    }
    upstream.write_all(b"\r\n")?;

    let mut up_reader = BufReader::new(upstream);
    let mut status = String::new();
    up_reader.read_line(&mut status)?;
    let mut resp_headers = Vec::<(String, String)>::new();
    loop {
        let mut line = String::new();
        up_reader.read_line(&mut line)?;
        if line == "\r\n" || line == "\n" || line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_string();
            let val = v.trim().to_string();
            if key.eq_ignore_ascii_case("set-cookie") {
                let name = val.split('=').next().unwrap_or("").trim();
                if !name.is_empty() {
                    let decision = report(&engine, CookieAction::Set, &host_only, name, "http");
                    if matches!(decision, DecisionKind::Deny | DecisionKind::Strip) {
                        continue;
                    }
                }
            }
            resp_headers.push((key, val));
        }
    }

    client.write_all(status.as_bytes())?;
    for (k, v) in &resp_headers {
        write!(client, "{k}: {v}\r\n")?;
    }
    client.write_all(b"\r\n")?;
    let _ = std::io::copy(&mut up_reader, &mut client);
    Ok(())
}

fn cookie_names(header: &str) -> Vec<String> {
    header
        .split(';')
        .filter_map(|part| {
            let name = part.split('=').next()?.trim();
            (!name.is_empty()).then(|| name.to_string())
        })
        .collect()
}

fn report(
    engine: &Arc<Mutex<Engine>>,
    action: CookieAction,
    host: &str,
    name: &str,
    scheme: &str,
) -> DecisionKind {
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let event = CookieEvent {
        id: format!("proxy-{scheme}-{ts_ms}-{name}"),
        ts_ms,
        action,
        party: CookieParty::First,
        scheme: scheme.into(),
        host: host.into(),
        name: name.into(),
        value_redacted: true,
        attributes: CookieAttributes::default(),
    };
    match engine.lock() {
        Ok(mut eng) => eng
            .handle_event(event)
            .map(|d| d.kind)
            .unwrap_or(DecisionKind::Allow),
        Err(_) => DecisionKind::Allow,
    }
}
