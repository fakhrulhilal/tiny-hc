//! Tiny HTTP(S) health check.
//!
//! Sends a `GET` request to the given URL and exits with `0` when the server
//! answers with a `2xx` status (and, optionally, the body contains the
//! expected text). Everything else exits with `1`, as Docker expects from a
//! `HEALTHCHECK` command.
//!
//! The HTTP/1.1 client is hand written on top of `std` to keep the binary
//! small. HTTPS support (a trimmed mbedTLS that never verifies certificates)
//! is only compiled in with the `tls` feature.

use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::process::ExitCode;
use std::time::{Duration, Instant};

#[cfg(feature = "tls")]
mod tls;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_TIMEOUT_SECS: f64 = 5.0;
/// Upper bound of response headers we are willing to buffer.
const MAX_HEAD: usize = 64 * 1024;
/// Upper bound of response body we are willing to buffer for `--expect-response`.
const MAX_BODY: usize = 1024 * 1024;

#[derive(Debug, PartialEq)]
pub struct Options {
    pub url: String,
    pub timeout: Duration,
    pub basic_auth: Option<String>,
    pub expect_response: Option<String>,
}

#[derive(Debug, PartialEq)]
enum Command {
    Check(Options),
    Help,
    Version,
}

/// Entry point shared by both binaries.
pub fn run(bin: &str) -> ExitCode {
    let result = parse_args(std::env::args().skip(1)).and_then(|cmd| match cmd {
        Command::Help => {
            print!("{}", help(bin));
            Ok(())
        }
        Command::Version => {
            println!("{bin} {VERSION}");
            Ok(())
        }
        Command::Check(opts) => check(&opts),
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{bin}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn help(bin: &str) -> String {
    let https = if cfg!(feature = "tls") {
        "http:// or https:// (certificates are not verified)"
    } else {
        "http:// (this build has no TLS support)"
    };
    format!(
        "{bin} {VERSION} - tiny HTTP health check

Usage: {bin} [OPTIONS] <URL>

  URL must start with {https}.
  Exits 0 when the response status is 2xx, 1 otherwise.

Options:
  -t, --timeout <SECONDS>         Whole request timeout, fractions allowed [default: 5]
      --basic-auth <USER:PASS>    Send HTTP basic authentication
      --expect-response <TEXT>    Also require the response body to contain TEXT
  -h, --help                      Print help
  -V, --version                   Print version
"
    )
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let mut url = None;
    let mut timeout = DEFAULT_TIMEOUT_SECS;
    let mut basic_auth = None;
    let mut expect_response = None;
    let mut only_positional = false;

    while let Some(arg) = args.next() {
        if only_positional || !arg.starts_with('-') || arg == "-" {
            if url.replace(arg).is_some() {
                return Err("only one URL is allowed".into());
            }
            continue;
        }
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n.to_string(), Some(v.to_string())),
            _ => (arg, None),
        };
        let mut value = || inline.clone().or_else(|| args.next()).ok_or_else(|| format!("{name} requires a value"));
        match name.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "-V" | "--version" => return Ok(Command::Version),
            "-t" | "--timeout" => {
                let v = value()?;
                timeout = v.parse().ok().filter(|t: &f64| t.is_finite() && *t > 0.0).ok_or_else(|| format!("invalid timeout: {v}"))?;
            }
            "--basic-auth" => basic_auth = Some(value()?),
            "--expect-response" => expect_response = Some(value()?),
            "--" => only_positional = true,
            _ => return Err(format!("unknown option: {name} (see --help)")),
        }
    }

    Ok(Command::Check(Options {
        url: url.ok_or("missing URL (see --help)")?,
        timeout: Duration::from_secs_f64(timeout),
        basic_auth,
        expect_response,
    }))
}

#[derive(Debug, PartialEq)]
struct Url {
    https: bool,
    host: String,
    port: u16,
    /// Path and query, always starting with `/`.
    target: String,
    /// Decoded `user:pass` from the URL, if any.
    userinfo: Option<String>,
}

impl Url {
    fn parse(s: &str) -> Result<Url, String> {
        let invalid = || format!("invalid URL: {s}");
        let (scheme, rest) = s.split_once("://").ok_or_else(invalid)?;
        let https = match scheme.to_ascii_lowercase().as_str() {
            "http" => false,
            "https" => true,
            _ => return Err(format!("unsupported scheme {scheme}:// (only http and https)")),
        };
        let (authority, target) = match rest.find(['/', '?', '#']) {
            Some(i) => rest.split_at(i),
            None => (rest, ""),
        };
        let target = target.split('#').next().unwrap_or_default();
        let target = if target.starts_with('/') { target.to_string() } else { format!("/{target}") };

        let (userinfo, hostport) = match authority.rsplit_once('@') {
            Some((u, h)) => (Some(percent_decode(u)), h),
            None => (None, authority),
        };
        let (host, port) = if let Some(v6) = hostport.strip_prefix('[') {
            let (host, after) = v6.split_once(']').ok_or_else(invalid)?;
            (host, after.strip_prefix(':'))
        } else {
            match hostport.rsplit_once(':') {
                Some((h, p)) => (h, Some(p)),
                None => (hostport, None),
            }
        };
        if host.is_empty() {
            return Err(invalid());
        }
        let port = match port {
            Some(p) => p.parse().ok().filter(|p| *p != 0).ok_or_else(invalid)?,
            None if https => 443,
            None => 80,
        };
        Ok(Url { https, host: host.to_string(), port, target, userinfo })
    }

    fn host_header(&self) -> String {
        let host = if self.host.contains(':') { format!("[{}]", self.host) } else { self.host.clone() };
        if self.port == if self.https { 443 } else { 80 } { host } else { format!("{host}:{}", self.port) }
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = b.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok());
        match (b[i], hex) {
            (b'%', Some(v)) => {
                out.push(v);
                i += 3;
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn base64(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for c in input.chunks(3) {
        let n = u32::from(c[0]) << 16 | u32::from(*c.get(1).unwrap_or(&0)) << 8 | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            out.push(if i <= c.len() { T[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// TCP stream that enforces one overall deadline on every read and write.
struct Conn {
    sock: TcpStream,
    deadline: Instant,
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "timed out"))
}

/// Socket timeouts surface as `WouldBlock` on Unix; report them as timeouts.
fn timed_out(e: io::Error) -> io::Error {
    if e.kind() == io::ErrorKind::WouldBlock { io::Error::new(io::ErrorKind::TimedOut, "timed out") } else { e }
}

impl Read for Conn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.sock.set_read_timeout(Some(remaining(self.deadline)?))?;
        self.sock.read(buf).map_err(timed_out)
    }
}

impl Write for Conn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.sock.set_write_timeout(Some(remaining(self.deadline)?))?;
        self.sock.write(buf).map_err(timed_out)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.sock.flush()
    }
}

fn connect(url: &Url, deadline: Instant) -> Result<Conn, String> {
    let addrs = (url.host.as_str(), url.port).to_socket_addrs().map_err(|e| format!("cannot resolve {}: {e}", url.host))?;
    let mut last = format!("no address found for {}", url.host);
    for addr in addrs {
        let left = remaining(deadline).map_err(|e| format!("connect to {}: {e}", url.host_header()))?;
        match TcpStream::connect_timeout(&addr, left) {
            Ok(sock) => {
                let _ = sock.set_nodelay(true);
                return Ok(Conn { sock, deadline });
            }
            Err(e) => last = format!("connect to {addr}: {e}"),
        }
    }
    Err(last)
}

trait Stream: Read + Write {}
impl<T: Read + Write> Stream for T {}

/// Runs the health check.
pub fn check(opts: &Options) -> Result<(), String> {
    let url = Url::parse(&opts.url)?;
    if url.https && !cfg!(feature = "tls") {
        return Err("https is not supported by this binary, use tiny-hc-tls".into());
    }
    let deadline = Instant::now() + opts.timeout;
    let conn = connect(&url, deadline)?;

    let mut stream: Box<dyn Stream> = if url.https {
        #[cfg(feature = "tls")]
        {
            Box::new(tls::wrap(conn, &url.host)?)
        }
        #[cfg(not(feature = "tls"))]
        unreachable!()
    } else {
        Box::new(conn)
    };

    let mut req = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: tiny-hc/{VERSION}\r\nAccept: */*\r\nConnection: close\r\n",
        url.target,
        url.host_header()
    );
    if let Some(auth) = opts.basic_auth.as_ref().or(url.userinfo.as_ref()) {
        req += &format!("Authorization: Basic {}\r\n", base64(auth.as_bytes()));
    }
    req += "\r\n";
    stream.write_all(req.as_bytes()).and_then(|()| stream.flush()).map_err(|e| format!("send request: {e}"))?;

    let (status, body) = read_response(&mut stream, opts.expect_response.is_some())?;
    if !(200..300).contains(&status) {
        return Err(format!("unhealthy: HTTP status {status}"));
    }
    if let Some(expected) = &opts.expect_response
        && !contains(&body, expected.as_bytes())
    {
        return Err(format!("unhealthy: response does not contain {expected:?}"));
    }
    Ok(())
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w == needle)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Reads one chunk; a peer closing the connection (TLS without close_notify
/// included) counts as end of stream.
fn read_some(stream: &mut dyn Read, buf: &mut Vec<u8>) -> Result<bool, String> {
    let mut chunk = [0u8; 8192];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => return Ok(false),
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                return Ok(true);
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
            Err(e) => return Err(format!("read response: {e}")),
        }
    }
}

/// Returns the status code and, when `want_body` is set, the (de-chunked) body.
fn read_response(stream: &mut dyn Read, want_body: bool) -> Result<(u16, Vec<u8>), String> {
    let mut buf = Vec::new();
    let head_end = loop {
        if let Some(i) = find(&buf, b"\r\n\r\n") {
            break i;
        }
        if buf.len() > MAX_HEAD {
            return Err("response headers too large".into());
        }
        if !read_some(stream, &mut buf)? {
            return Err(if buf.is_empty() { "empty response".into() } else { "incomplete response headers".into() });
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .filter(|l| l.starts_with("HTTP/"))
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or("malformed HTTP status line")?;
    if !want_body {
        return Ok((status, Vec::new()));
    }

    let mut content_length = None;
    let mut chunked = false;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            let (k, v) = (k.trim(), v.trim());
            if k.eq_ignore_ascii_case("content-length") {
                content_length = v.parse::<usize>().ok();
            } else if k.eq_ignore_ascii_case("transfer-encoding") {
                chunked = v.to_ascii_lowercase().contains("chunked");
            }
        }
    }

    let mut body = buf.split_off(head_end + 4);
    loop {
        let done = match (chunked, content_length) {
            (true, _) => dechunk(&body).1,
            (false, Some(n)) => body.len() >= n,
            (false, None) => false,
        };
        if done || body.len() > MAX_BODY || !read_some(stream, &mut body)? {
            break;
        }
    }
    if chunked {
        body = dechunk(&body).0;
    } else if let Some(n) = content_length {
        body.truncate(n);
    }
    Ok((status, body))
}

/// Decodes a chunked body. Returns the data decoded so far and whether the
/// terminating zero-size chunk was seen.
fn dechunk(mut raw: &[u8]) -> (Vec<u8>, bool) {
    let mut out = Vec::new();
    while let Some(eol) = find(raw, b"\r\n") {
        let line = String::from_utf8_lossy(&raw[..eol]);
        let Ok(size) = usize::from_str_radix(line.split(';').next().unwrap_or_default().trim(), 16) else {
            break;
        };
        if size == 0 {
            return (out, true);
        }
        raw = &raw[eol + 2..];
        let take = size.min(raw.len());
        out.extend_from_slice(&raw[..take]);
        if raw.len() < size + 2 {
            break;
        }
        raw = &raw[size + 2..];
    }
    (out, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn args(a: &[&str]) -> Result<Command, String> {
        parse_args(a.iter().map(|s| s.to_string()))
    }

    fn opts(url: &str) -> Options {
        Options { url: url.into(), timeout: Duration::from_secs(5), basic_auth: None, expect_response: None }
    }

    #[test]
    fn parses_arguments() {
        let got = args(&["--timeout=1.5", "--basic-auth", "u:p", "--expect-response", "-ok-", "http://x"]).unwrap();
        assert_eq!(
            got,
            Command::Check(Options {
                url: "http://x".into(),
                timeout: Duration::from_millis(1500),
                basic_auth: Some("u:p".into()),
                expect_response: Some("-ok-".into()),
            })
        );
        assert_eq!(args(&["http://x"]).unwrap(), Command::Check(opts("http://x")));
        assert_eq!(args(&["-h"]).unwrap(), Command::Help);
        assert_eq!(args(&["--version"]).unwrap(), Command::Version);
        assert!(args(&[]).is_err());
        assert!(args(&["--timeout", "0", "http://x"]).is_err());
        assert!(args(&["--timeout"]).is_err());
        assert!(args(&["--nope", "http://x"]).is_err());
        assert!(args(&["http://x", "http://y"]).is_err());
        assert!(args(&["--always-trust-certificate", "http://x"]).is_err());
    }

    #[test]
    fn parses_urls() {
        let u = Url::parse("https://us%40r:p%3Ass@Example.com:8443/health?x=1#frag").unwrap();
        assert_eq!(
            u,
            Url { https: true, host: "Example.com".into(), port: 8443, target: "/health?x=1".into(), userinfo: Some("us@r:p:ss".into()) }
        );
        assert_eq!(u.host_header(), "Example.com:8443");

        let u = Url::parse("http://[::1]?a").unwrap();
        assert_eq!((u.host.as_str(), u.port, u.target.as_str()), ("::1", 80, "/?a"));
        assert_eq!(u.host_header(), "[::1]");
        assert_eq!(Url::parse("HTTPS://h").unwrap().port, 443);

        for bad in ["ftp://h", "tcp://h:1", "h:80", "http://", "http://h:0", "http://h:x", "http://[::1"] {
            assert!(Url::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn encodes_base64() {
        for (i, o) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("user:pass", "dXNlcjpwYXNz")] {
            assert_eq!(base64(i.as_bytes()), o);
        }
    }

    #[test]
    fn decodes_chunked() {
        assert_eq!(dechunk(b"4\r\nWiki\r\n5;x=y\r\npedia\r\n0\r\n\r\n"), (b"Wikipedia".to_vec(), true));
        assert_eq!(dechunk(b"4\r\nWiki\r\n5\r\npe"), (b"Wikipe".to_vec(), false));
    }

    /// Serves one canned response and returns the URL plus a handle yielding the raw request.
    fn serve(response: &'static str) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/health", listener.local_addr().unwrap().port());
        let handle = thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut req = Vec::new();
            while find(&req, b"\r\n\r\n").is_none() {
                let mut b = [0u8; 1024];
                let n = s.read(&mut b).unwrap();
                req.extend_from_slice(&b[..n]);
            }
            s.write_all(response.as_bytes()).unwrap();
            String::from_utf8(req).unwrap()
        });
        (url, handle)
    }

    #[test]
    fn healthy_response() {
        let (url, h) = serve("HTTP/1.1 204 No Content\r\n\r\n");
        let mut o = opts(&url);
        o.basic_auth = Some("user:pass".into());
        assert_eq!(check(&o), Ok(()));
        let req = h.join().unwrap();
        assert!(req.starts_with("GET /health HTTP/1.1\r\n"), "{req}");
        assert!(req.contains("\r\nAuthorization: Basic dXNlcjpwYXNz\r\n"), "{req}");
    }

    #[test]
    fn unhealthy_status() {
        let (url, h) = serve("HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n");
        assert_eq!(check(&opts(&url)), Err("unhealthy: HTTP status 503".into()));
        h.join().unwrap();
    }

    #[test]
    fn expect_response_content_length() {
        let (url, h) = serve("HTTP/1.1 200 OK\r\nContent-Length: 15\r\n\r\n{\"status\":\"UP\"}");
        let mut o = opts(&url);
        o.expect_response = Some("\"UP\"".into());
        assert_eq!(check(&o), Ok(()));
        h.join().unwrap();

        let (url, h) = serve("HTTP/1.1 200 OK\r\nContent-Length: 17\r\n\r\n{\"status\":\"DOWN\"}");
        o.url = url;
        assert!(check(&o).is_err());
        h.join().unwrap();
    }

    #[test]
    fn expect_response_chunked_and_eof() {
        let (url, h) = serve("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nhea\r\n5\r\nlthy!\r\n0\r\n\r\n");
        let mut o = opts(&url);
        o.expect_response = Some("healthy".into());
        assert_eq!(check(&o), Ok(()));
        h.join().unwrap();

        let (url, h) = serve("HTTP/1.0 200 OK\r\n\r\nall healthy");
        o.url = url;
        assert_eq!(check(&o), Ok(()));
        h.join().unwrap();
    }

    #[test]
    fn times_out() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut o = opts(&format!("http://{}/", listener.local_addr().unwrap()));
        o.timeout = Duration::from_millis(200);
        let started = Instant::now();
        let err = check(&o).unwrap_err();
        assert!(err.contains("timed out"), "{err}");
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn rejects_unreachable() {
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        assert!(check(&opts(&format!("http://127.0.0.1:{port}/"))).is_err());
    }

    #[cfg(not(feature = "tls"))]
    #[test]
    fn https_needs_tls_build() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let err = check(&opts(&format!("https://{}/", listener.local_addr().unwrap()))).unwrap_err();
        assert!(err.contains("tiny-hc-tls"), "{err}");
    }
}
