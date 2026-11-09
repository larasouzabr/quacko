use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(1);
pub const DEFAULT_PORT: u16 = 4747;

pub fn port(env: Option<&str>, config: Option<&str>) -> u16 {
    let from_env = env.and_then(|v| v.trim().parse::<u16>().ok());
    let from_config = config
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        .and_then(|v| v.get("port").and_then(serde_json::Value::as_u64))
        .and_then(|p| u16::try_from(p).ok());
    from_env
        .or(from_config)
        .filter(|p| *p > 0)
        .unwrap_or(DEFAULT_PORT)
}

pub fn request(port: u16, body: &str) -> String {
    format!(
        "POST /notify HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

pub fn post(port: u16, body: &str) {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, TIMEOUT) else {
        return;
    };
    let _ = stream.set_write_timeout(Some(TIMEOUT));
    let _ = stream.set_read_timeout(Some(TIMEOUT));
    if stream.write_all(request(port, body).as_bytes()).is_ok() {
        let mut sink = [0u8; 512];
        let _ = stream.read(&mut sink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn porta_da_variavel_da_config_ou_padrao() {
        assert_eq!(port(Some("5000"), Some(r#"{"port":6000}"#)), 5000);
        assert_eq!(port(None, Some(r#"{"port":6000}"#)), 6000);
        assert_eq!(port(Some("abc"), Some("não é json")), DEFAULT_PORT);
        assert_eq!(port(None, Some(r#"{"port":70000}"#)), DEFAULT_PORT);
        assert_eq!(port(Some("0"), None), DEFAULT_PORT);
    }

    #[test]
    fn envia_o_json_para_o_app() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut got = String::new();
            let mut buf = [0u8; 1024];
            while !got.ends_with('}') {
                let n = conn.read(&mut buf).unwrap();
                got.push_str(std::str::from_utf8(&buf[..n]).unwrap());
            }
            conn.write_all(b"HTTP/1.1 204 No Content\r\n\r\n").unwrap();
            got
        });
        post(port, r#"{"status":"ok"}"#);
        let got = server.join().unwrap();
        assert!(got.starts_with("POST /notify HTTP/1.1\r\n"));
        assert!(got.contains("Content-Type: application/json"));
        assert!(got.ends_with("\r\n\r\n{\"status\":\"ok\"}"));
    }

    #[test]
    fn app_fechado_nao_trava() {
        let free = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let start = std::time::Instant::now();
        post(free, "{}");
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
