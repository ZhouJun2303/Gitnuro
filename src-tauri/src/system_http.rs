//! Windows' HTTP stack shares the system proxy/PAC and certificate store with browsers.
//! Link the OS library instead of bundling another TLS or download runtime.

use std::ffi::c_void;
use std::io::{self, Read};
use std::ptr::{null, null_mut};

type RawHandle = *mut c_void;

#[link(name = "winhttp")]
extern "system" {
    fn WinHttpOpen(agent: *const u16, access: u32, proxy: *const u16, bypass: *const u16, flags: u32) -> RawHandle;
    fn WinHttpConnect(session: RawHandle, host: *const u16, port: u16, reserved: u32) -> RawHandle;
    fn WinHttpOpenRequest(connection: RawHandle, verb: *const u16, path: *const u16, version: *const u16, referer: *const u16, accept: *const *const u16, flags: u32) -> RawHandle;
    fn WinHttpSetTimeouts(handle: RawHandle, resolve: i32, connect: i32, send: i32, receive: i32) -> i32;
    fn WinHttpSetOption(handle: RawHandle, option: u32, buffer: *const c_void, length: u32) -> i32;
    fn WinHttpSendRequest(request: RawHandle, headers: *const u16, headers_len: u32, optional: *const c_void, optional_len: u32, total_len: u32, context: usize) -> i32;
    fn WinHttpReceiveResponse(request: RawHandle, reserved: *mut c_void) -> i32;
    fn WinHttpQueryHeaders(request: RawHandle, level: u32, name: *const u16, buffer: *mut c_void, length: *mut u32, index: *mut u32) -> i32;
    fn WinHttpReadData(request: RawHandle, buffer: *mut c_void, length: u32, read: *mut u32) -> i32;
    fn WinHttpCloseHandle(handle: RawHandle) -> i32;
}

struct Handle(RawHandle);

impl Handle {
    fn new(raw: RawHandle) -> Result<Self, String> {
        if raw.is_null() { Err(network_error()) } else { Ok(Self(raw)) }
    }
}

impl Drop for Handle {
    fn drop(&mut self) { unsafe { WinHttpCloseHandle(self.0); } }
}

pub struct Response {
    // Rust drops fields in this order; child handles must close before their parents.
    request: Handle,
    _connection: Handle,
    _session: Handle,
    pub total: u64,
}

fn wide(value: &str) -> Vec<u16> { value.encode_utf16().chain(Some(0)).collect() }

fn network_error() -> String {
    crate::http::coded("network", &format!("Windows HTTP: {}", io::Error::last_os_error()))
}

impl Response {
    pub fn get(url: &str, read_secs: u64) -> Result<Self, String> {
        let (secure, rest) = if let Some(rest) = url.strip_prefix("https://") { (true, rest) }
            else if let Some(rest) = url.strip_prefix("http://") { (false, rest) }
            else { return Err(crate::http::coded("network", "invalid download URL")); };
        let (authority, target) = rest.split_once('/').map(|(host, path)| (host, format!("/{path}"))).unwrap_or((rest, "/".into()));
        if authority.is_empty() || authority.contains(['@', '\0', '\r', '\n']) || target.contains(['\0', '\r', '\n']) {
            return Err(crate::http::coded("network", "invalid download URL"));
        }
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) => (host, port.parse::<u16>().map_err(|_| crate::http::coded("network", "invalid port"))?),
            None => (authority, if secure { 443 } else { 80 }),
        };
        let agent = wide(concat!("AweGit/", env!("CARGO_PKG_VERSION")));
        let host = wide(host);
        let target = wide(&target);
        let get = wide("GET");
        unsafe {
            // AUTOMATIC_PROXY includes the signed-in user's proxy and PAC configuration.
            let session = Handle::new(WinHttpOpen(agent.as_ptr(), 4, null(), null(), 0))?;
            if WinHttpSetTimeouts(session.0, 20_000, 20_000, 20_000, read_secs.clamp(1, 60) as i32 * 1000) == 0 { return Err(network_error()); }
            let connection = Handle::new(WinHttpConnect(session.0, host.as_ptr(), port, 0))?;
            let request = Handle::new(WinHttpOpenRequest(connection.0, get.as_ptr(), target.as_ptr(), null(), null(), null(), if secure { 0x0080_0000 } else { 0 }))?;
            // Follow GitHub's CDN redirects, while refusing HTTPS -> HTTP downgrades.
            let policy: u32 = 1;
            if WinHttpSetOption(request.0, 88, &policy as *const _ as *const c_void, 4) == 0 { return Err(network_error()); }
            if WinHttpSendRequest(request.0, null(), 0, null(), 0, 0, 0) == 0 || WinHttpReceiveResponse(request.0, null_mut()) == 0 { return Err(network_error()); }
            let mut status: u32 = 0;
            let mut length: u32 = 4;
            if WinHttpQueryHeaders(request.0, 19 | 0x2000_0000, null(), &mut status as *mut _ as *mut c_void, &mut length, null_mut()) == 0 { return Err(network_error()); }
            if !(200..300).contains(&status) { return Err(crate::http::coded("network", &format!("HTTP {status}"))); }
            let mut value = [0u16; 32];
            let mut length = std::mem::size_of_val(&value) as u32;
            let total = if WinHttpQueryHeaders(request.0, 5, null(), value.as_mut_ptr() as *mut c_void, &mut length, null_mut()) != 0 {
                String::from_utf16_lossy(&value[..value.iter().position(|ch| *ch == 0).unwrap_or(value.len())]).trim().parse().unwrap_or(0)
            } else { 0 };
            Ok(Self { request, _connection: connection, _session: session, total })
        }
    }
}

impl Read for Response {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() { return Ok(0); }
        let mut read: u32 = 0;
        let ok = unsafe { WinHttpReadData(self.request.0, buf.as_mut_ptr() as *mut c_void, buf.len().min(u32::MAX as usize) as u32, &mut read) };
        if ok == 0 { Err(io::Error::last_os_error()) } else { Ok(read as usize) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    #[test]
    fn system_download_streams_redirects_and_reports_http_errors() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = server.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            for body in ["HTTP/1.1 302 Found\r\nLocation: /asset\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                "HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\ninstaller",
                "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"] {
                let (mut socket, _) = server.accept().unwrap();
                let mut request = [0; 2048];
                socket.read(&mut request).unwrap();
                socket.write_all(body.as_bytes()).unwrap();
            }
        });
        let mut response = Response::get(&format!("http://{address}/redirect"), 2).unwrap();
        assert_eq!(response.total, 9);
        let mut body = String::new();
        response.read_to_string(&mut body).unwrap();
        assert_eq!(body, "installer");
        assert!(Response::get(&format!("http://{address}/denied"), 2).err().unwrap().contains("HTTP 403"));
        worker.join().unwrap();
    }
}
