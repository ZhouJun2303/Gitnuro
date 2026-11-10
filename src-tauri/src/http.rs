//! Shared HTTP client for updates, AI, and the forges.
//!
//! Proxy, CA, and SSL verification come from the settings captured by
//! [`with_net`]. Callers set that for the duration of a command. The default
//! is a direct connection, so unit tests do not read the user's settings file.

use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;

#[cfg(test)]
use std::io::{Read, Write};
#[cfg(test)]
use std::net::TcpListener;

use crate::settings::Settings;

thread_local! {
    static NET: RefCell<NetOptions> = RefCell::new(NetOptions::direct());
}

#[derive(Clone, Debug)]
pub struct NetOptions {
    pub proxy: String,
    pub ssl_verify: bool,
    pub ssl_ca_file: String,
    pub timeout_secs: u64,
}

impl NetOptions {
    pub fn direct() -> Self {
        Self {
            proxy: String::new(),
            ssl_verify: true,
            ssl_ca_file: String::new(),
            timeout_secs: 20,
        }
    }

    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            proxy: compose_proxy(settings),
            ssl_verify: settings.ssl_verify,
            ssl_ca_file: settings.ssl_ca_file.trim().to_string(),
            timeout_secs: 20,
        }
    }
}

#[derive(Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

pub fn with_net<T>(options: NetOptions, body: impl FnOnce() -> T) -> T {
    let previous = NET.with(|slot| slot.replace(options));
    let _restore = Restore(previous);
    body()
}

struct Restore(NetOptions);

impl Drop for Restore {
    fn drop(&mut self) {
        let previous = std::mem::replace(&mut self.0, NetOptions::direct());
        NET.with(|slot| {
            slot.replace(previous);
        });
    }
}

fn current() -> NetOptions {
    NET.with(|slot| slot.borrow().clone())
}

pub fn coded(code: &str, detail: &str) -> String {
    let detail = scrub(detail).replace(['\n', '\r'], " ");
    let detail = clip(&detail);
    format!("awegit:{code}:{detail}")
}

/// The same proxy string Git receives in `http_proxy`.
pub fn compose_proxy(settings: &Settings) -> String {
    let host = settings.proxy_host.trim();
    if settings.proxy_enabled && !host.is_empty() {
        let scheme = if settings.proxy_type.eq_ignore_ascii_case("socks") {
            "socks5"
        } else {
            "http"
        };
        let port = if settings.proxy_port > 0 {
            format!(":{}", settings.proxy_port)
        } else {
            String::new()
        };
        let auth = if settings.proxy_user.trim().is_empty() {
            String::new()
        } else {
            format!("{}:{}@", encode_userinfo(settings.proxy_user.trim()), encode_userinfo(&settings.proxy_password))
        };
        return format!("{scheme}://{auth}{host}{port}");
    }
    let proxy = settings.proxy.trim();
    if proxy.is_empty() {
        return String::new();
    }
    if settings.proxy_user.trim().is_empty() {
        return proxy.to_string();
    }
    let user = encode_userinfo(settings.proxy_user.trim());
    let password = encode_userinfo(&settings.proxy_password);
    if let Some((scheme, rest)) = proxy.split_once("://") {
        format!("{scheme}://{user}:{password}@{rest}")
    } else {
        format!("http://{user}:{password}@{proxy}")
    }
}

pub fn encode_userinfo(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub fn send(method: &str, url: &str, headers: &[(&str, &str)], body: Option<&str>) -> Result<HttpResponse, String> {
    let net = current();
    let agent = build_agent(net.timeout_secs.max(1), false)?;
    let mut request = agent.request(method, url);
    for (name, value) in headers {
        request = request.set(name, value);
    }
    let result = match body {
        Some(text) => request.send_string(text),
        None => request.call(),
    };
    match result {
        Ok(response) => {
            let status = response.status();
            let body = response.into_string().unwrap_or_default();
            Ok(HttpResponse { status, body })
        }
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            if status == 401 || status == 403 {
                let detail = if body.trim().is_empty() {
                    format!("HTTP {status}")
                } else {
                    clip(&scrub(&body))
                };
                Err(coded("auth", &detail))
            } else {
                Ok(HttpResponse { status, body })
            }
        }
        Err(error) => Err(coded("network", &error.to_string())),
    }
}

pub fn agent(read_secs: u64, https_only: bool) -> Result<ureq::Agent, String> {
    build_agent(read_secs, https_only)
}

fn build_agent(read_secs: u64, https_only: bool) -> Result<ureq::Agent, String> {
    let net = current();
    let mut builder = ureq::AgentBuilder::new()
        .https_only(https_only)
        .redirects(if https_only { 10 } else { 5 })
        .timeout_connect(Duration::from_secs(net.timeout_secs.clamp(1, 20)))
        .timeout_read(Duration::from_secs(read_secs.max(1)))
        .timeout_write(Duration::from_secs(20))
        .user_agent("AweGit");
    if !net.proxy.trim().is_empty() {
        let proxy = ureq::Proxy::new(net.proxy.trim()).map_err(|error| coded("network", &error.to_string()))?;
        builder = builder.proxy(proxy);
    }
    if let Some(config) = tls_config(&net)? {
        builder = builder.tls_config(Arc::new(config));
    }
    Ok(builder.build())
}

fn tls_config(net: &NetOptions) -> Result<Option<ureq::rustls::ClientConfig>, String> {
    if !net.ssl_verify {
        let config = tls_builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAny))
            .with_no_client_auth();
        return Ok(Some(config));
    }
    if net.ssl_ca_file.trim().is_empty() {
        return Ok(None);
    }
    let bytes = std::fs::read(net.ssl_ca_file.trim()).map_err(|error| coded("network", &format!("CA file: {error}")))?;
    let certs = pem_certs(&bytes);
    if certs.is_empty() {
        return Err(coded("network", "CA file has no certificate"));
    }
    let mut roots = ureq::rustls::RootCertStore::empty();
    let (valid, _) = roots.add_parsable_certificates(certs);
    if valid == 0 {
        return Err(coded("network", "CA file has no certificate"));
    }
    Ok(Some(tls_builder().with_root_certificates(roots).with_no_client_auth()))
}

fn tls_builder() -> ureq::rustls::ConfigBuilder<ureq::rustls::ClientConfig, ureq::rustls::WantsVerifier> {
    ureq::rustls::ClientConfig::builder_with_provider(ureq::rustls::crypto::ring::default_provider().into())
        .with_protocol_versions(&[&ureq::rustls::version::TLS12, &ureq::rustls::version::TLS13])
        .expect("ring supports TLS 1.2 and 1.3")
}

#[derive(Debug)]
struct AcceptAny;

impl ureq::rustls::client::danger::ServerCertVerifier for AcceptAny {
    fn verify_server_cert(
        &self,
        _end_entity: &ureq::rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[ureq::rustls::pki_types::CertificateDer<'_>],
        _server_name: &ureq::rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: ureq::rustls::pki_types::UnixTime,
    ) -> Result<ureq::rustls::client::danger::ServerCertVerified, ureq::rustls::Error> {
        Ok(ureq::rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &ureq::rustls::pki_types::CertificateDer<'_>,
        dss: &ureq::rustls::DigitallySignedStruct,
    ) -> Result<ureq::rustls::client::danger::HandshakeSignatureValid, ureq::rustls::Error> {
        ureq::rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &ureq::rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &ureq::rustls::pki_types::CertificateDer<'_>,
        dss: &ureq::rustls::DigitallySignedStruct,
    ) -> Result<ureq::rustls::client::danger::HandshakeSignatureValid, ureq::rustls::Error> {
        ureq::rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &ureq::rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<ureq::rustls::SignatureScheme> {
        ureq::rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn pem_certs(bytes: &[u8]) -> Vec<ureq::rustls::pki_types::CertificateDer<'static>> {
    let text = String::from_utf8_lossy(bytes);
    let begin = "-----BEGIN CERTIFICATE-----";
    let end = "-----END CERTIFICATE-----";
    let mut rest = text.as_ref();
    let mut out = Vec::new();
    while let Some(start) = rest.find(begin) {
        rest = &rest[start + begin.len()..];
        let Some(stop) = rest.find(end) else { break };
        let encoded: String = rest[..stop].chars().filter(|ch| !ch.is_whitespace()).collect();
        rest = &rest[stop + end.len()..];
        if let Ok(der) = b64_decode(&encoded) {
            out.push(ureq::rustls::pki_types::CertificateDer::from(der));
        }
    }
    out
}

fn b64_decode(text: &str) -> Result<Vec<u8>, ()> {
    fn value(byte: u8) -> Result<u8, ()> {
        match byte {
            b'A'..=b'Z' => Ok(byte - b'A'),
            b'a'..=b'z' => Ok(byte - b'a' + 26),
            b'0'..=b'9' => Ok(byte - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(()),
        }
    }
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index + 4 <= bytes.len() {
        let (a, b, c, d) = (bytes[index], bytes[index + 1], bytes[index + 2], bytes[index + 3]);
        let av = value(a)?;
        let bv = value(b)?;
        out.push((av << 2) | (bv >> 4));
        if c != b'=' {
            let cv = value(c)?;
            out.push((bv << 4) | (cv >> 2));
            if d != b'=' {
                let dv = value(d)?;
                out.push((cv << 6) | dv);
            }
        }
        index += 4;
    }
    Ok(out)
}

fn scrub(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = ["https://", "http://", "socks5://", "socks://"]
        .iter()
        .filter_map(|scheme| rest.find(scheme).map(|at| (at, *scheme)))
        .min_by_key(|(at, _)| *at)
    {
        let (at, scheme) = start;
        out.push_str(&rest[..at]);
        out.push_str(scheme);
        rest = &rest[at + scheme.len()..];
        if let Some(end) = rest.find('@') {
            let auth = &rest[..end];
            if !auth.is_empty() && !auth.contains('/') {
                rest = &rest[end + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn clip(text: &str) -> String {
    let mut end = text.len().min(180);
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].trim().to_string()
}

#[cfg(test)]
fn serve(handler: impl Fn(&str) -> Vec<u8> + Send + 'static, accepts: usize) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for _ in 0..accepts {
            let Ok((mut sock, _)) = listener.accept() else { break };
            sock.set_read_timeout(Some(Duration::from_secs(2))).ok();
            let mut buf = [0u8; 4096];
            let n = sock.read(&mut buf).unwrap_or(0);
            let text = String::from_utf8_lossy(&buf[..n]).to_string();
            let _ = sock.write_all(&handler(&text));
        }
    });
    port
}

#[cfg(test)]
fn http_response(status: u16, body: &str) -> Vec<u8> {
    format!("HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socks_proxy_encodes_userinfo() {
        let mut settings = Settings::default();
        settings.proxy_enabled = true;
        settings.proxy_type = "socks".into();
        settings.proxy_host = "10.0.0.1".into();
        settings.proxy_port = 1080;
        settings.proxy_user = "a b".into();
        settings.proxy_password = "p@ss".into();
        assert_eq!(compose_proxy(&settings), "socks5://a%20b:p%40ss@10.0.0.1:1080");
    }

    #[test]
    fn errors_drop_proxy_passwords() {
        let text = scrub("connect to http://user:s3cret@10.0.0.1:8080/path failed");
        assert!(!text.contains("s3cret"), "{text}");
        assert!(text.contains("10.0.0.1"), "{text}");

        let token = scrub("connect to https://ghp_exampletoken@github.com/org/repo failed");
        assert!(!token.contains("ghp_exampletoken"), "{token}");
        assert!(token.contains("github.com/org/repo"), "{token}");

        let path = scrub("see https://github.com/org/repo/path@v1 for details");
        assert!(path.contains("repo/path@v1"), "{path}");
    }

    #[test]
    fn tls_config_builds_without_verification_and_rejects_a_bad_ca() {
        let off = NetOptions { ssl_verify: false, ..NetOptions::direct() };
        assert!(tls_config(&off).unwrap().is_some());
        let dir = std::env::temp_dir().join(format!("awegit-ca-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ca.pem");
        std::fs::write(&path, b"not a certificate").unwrap();
        let bad = NetOptions {
            ssl_ca_file: path.display().to_string(),
            ..NetOptions::direct()
        };
        let error = tls_config(&bad).unwrap_err();
        assert!(error.starts_with("awegit:network:"), "{error}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn local_http_redirect_and_status_codes() {
        let land = serve(|_| http_response(200, "landed"), 1);
        let redirect = serve(
            move |_| format!("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{land}/land\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes(),
            1,
        );
        let response = send("GET", &format!("http://127.0.0.1:{redirect}/start"), &[], None).unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, "landed");

        let denied = serve(|_| http_response(401, "nope"), 1);
        let error = send("GET", &format!("http://127.0.0.1:{denied}/"), &[], None).unwrap_err();
        assert!(error.starts_with("awegit:auth:"), "{error}");
        assert!(error.contains("nope"), "{error}");

        let missing = serve(|_| http_response(500, "boom"), 1);
        let response = send("GET", &format!("http://127.0.0.1:{missing}/"), &[], None).unwrap();
        assert_eq!(response.status, 500);
        assert_eq!(response.body, "boom");
    }

    #[test]
    fn http_proxy_answers_without_forwarding() {
        let port = serve(|_| http_response(200, "proxied"), 1);
        let options = NetOptions {
            proxy: format!("http://127.0.0.1:{port}"),
            ..NetOptions::direct()
        };
        let response = with_net(options, || send("GET", "http://example.invalid/nope", &[], None)).unwrap();
        assert_eq!(response.body, "proxied");
        assert_eq!(response.status, 200);
    }

    #[test]
    fn a_silent_server_times_out_as_a_network_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = sock.read(&mut buf);
                std::thread::sleep(Duration::from_secs(3));
            }
        });
        let options = NetOptions { timeout_secs: 1, ..NetOptions::direct() };
        let started = std::time::Instant::now();
        let error = with_net(options, || send("GET", &format!("http://127.0.0.1:{port}/"), &[], None)).unwrap_err();
        assert!(error.starts_with("awegit:network:"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(3), "timed out too slowly");
    }
}
