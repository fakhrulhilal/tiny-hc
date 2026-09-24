//! HTTPS support on top of a trimmed mbedTLS (see csrc/ and build.rs).
//!
//! Like microcheck, the server certificate is never verified: a health check
//! usually talks to its own container over a self-signed certificate, and
//! skipping verification keeps the binary small (no root store, no chain
//! validation).

use std::ffi::{CString, c_int, c_uchar, c_void};
use std::io::{self, Read, Write};
use std::net::IpAddr;

#[repr(C)]
struct RawTls {
    _private: [u8; 0],
}

type SendFn = unsafe extern "C" fn(*mut c_void, *const c_uchar, usize) -> c_int;
type RecvFn = unsafe extern "C" fn(*mut c_void, *mut c_uchar, usize) -> c_int;

unsafe extern "C" {
    fn thc_tls_new(out: *mut *mut RawTls, sni: *const u8, io: *mut c_void, send: SendFn, recv: RecvFn) -> c_int;
    fn thc_tls_handshake(t: *mut RawTls) -> c_int;
    fn thc_tls_write(t: *mut RawTls, buf: *const c_uchar, len: usize) -> c_int;
    fn thc_tls_read(t: *mut RawTls, buf: *mut c_uchar, len: usize) -> c_int;
    fn thc_tls_free(t: *mut RawTls);
}

/// Returned by the I/O callbacks when the socket fails; the actual
/// `io::Error` is kept in [`Io::err`]. (MBEDTLS_ERR_NET_RECV_FAILED)
const ERR_IO: c_int = -0x004C;
const ERR_CONN_EOF: c_int = -0x7280;

struct Io<S> {
    sock: S,
    err: Option<io::Error>,
}

unsafe extern "C" fn send_cb<S: Write>(ctx: *mut c_void, buf: *const c_uchar, len: usize) -> c_int {
    // SAFETY: ctx is the `Io<S>` boxed in `TlsStream`, alive for the whole connection.
    let io = unsafe { &mut *ctx.cast::<Io<S>>() };
    let buf = unsafe { std::slice::from_raw_parts(buf, len.min(c_int::MAX as usize)) };
    match io.sock.write(buf) {
        Ok(n) => n as c_int,
        Err(e) => {
            io.err = Some(e);
            ERR_IO
        }
    }
}

unsafe extern "C" fn recv_cb<S: Read>(ctx: *mut c_void, buf: *mut c_uchar, len: usize) -> c_int {
    // SAFETY: see send_cb.
    let io = unsafe { &mut *ctx.cast::<Io<S>>() };
    let buf = unsafe { std::slice::from_raw_parts_mut(buf, len.min(c_int::MAX as usize)) };
    loop {
        match io.sock.read(buf) {
            Ok(n) => return n as c_int,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                io.err = Some(e);
                return ERR_IO;
            }
        }
    }
}

pub struct TlsStream<S> {
    raw: *mut RawTls,
    io: Box<Io<S>>,
}

impl<S> Drop for TlsStream<S> {
    fn drop(&mut self) {
        // SAFETY: raw came from thc_tls_new and is freed exactly once.
        unsafe { thc_tls_free(self.raw) }
    }
}

impl<S> TlsStream<S> {
    fn error(&mut self, code: c_int) -> io::Error {
        if let Some(e) = self.io.err.take() {
            return e;
        }
        if code == ERR_CONN_EOF {
            return io::ErrorKind::UnexpectedEof.into();
        }
        io::Error::other(describe(code))
    }
}

/// Connects TLS over `sock` and completes the handshake.
pub fn wrap<S: Read + Write>(sock: S, host: &str) -> Result<TlsStream<S>, String> {
    // SNI must be a DNS name, never an IP address.
    let sni = if host.parse::<IpAddr>().is_ok() { None } else { Some(CString::new(host).map_err(|_| "invalid host name")?) };
    let mut io = Box::new(Io { sock, err: None });
    let mut raw = std::ptr::null_mut();
    let ctx = (&mut *io as *mut Io<S>).cast::<c_void>();
    let sni_ptr = sni.as_ref().map_or(std::ptr::null(), |s| s.as_ptr().cast());
    // SAFETY: all pointers are valid; mbedTLS copies the host name; ctx outlives raw (both owned by TlsStream).
    let rc = unsafe { thc_tls_new(&mut raw, sni_ptr, ctx, send_cb::<S>, recv_cb::<S>) };
    if rc != 0 {
        return Err(format!("tls setup: {}", describe(rc)));
    }
    let mut stream = TlsStream { raw, io };
    // SAFETY: raw is a live context.
    let rc = unsafe { thc_tls_handshake(stream.raw) };
    if rc != 0 {
        return Err(format!("tls handshake: {}", stream.error(rc)));
    }
    Ok(stream)
}

impl<S> Read for TlsStream<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = buf.len().min(c_int::MAX as usize);
        // SAFETY: raw is live and buf is valid for len bytes.
        let rc = unsafe { thc_tls_read(self.raw, buf.as_mut_ptr(), len) };
        if rc >= 0 { Ok(rc as usize) } else { Err(self.error(rc)) }
    }
}

impl<S> Write for TlsStream<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let len = buf.len().min(c_int::MAX as usize);
        // SAFETY: raw is live and buf is valid for len bytes.
        let rc = unsafe { thc_tls_write(self.raw, buf.as_ptr(), len) };
        if rc >= 0 { Ok(rc as usize) } else { Err(self.error(rc)) }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Human readable text for the mbedTLS / PSA errors a client is likely to hit.
fn describe(code: c_int) -> String {
    let text = match code {
        -0x7780 => "fatal alert from server",
        -0x6E00 => "handshake failure (no common cipher suite, group or signature algorithm?)",
        -0x6E80 => "no common protocol version (TLS 1.2 or 1.3 required)",
        -0x7280 => "connection closed by server",
        -0x7700 => "unexpected message",
        -0x7300 | -0x7200 => "malformed message from server",
        -0x7180 => "bad record MAC",
        -0x7A00 => "unsupported or malformed server certificate",
        -0x3B00 => "unsupported server key (RSA keys above 4096 bits are not supported)",
        -0x6600 => "illegal parameter",
        -0x7500 => "unsupported extension",
        -141 => "out of memory",
        _ => return format!("mbedTLS error -0x{:04X}", code.unsigned_abs()),
    };
    text.to_string()
}
