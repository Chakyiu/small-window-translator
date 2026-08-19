use crate::config::Config;
use crate::AppCommand;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

pub fn start_server(port: u16, tx: Sender<AppCommand>) {
    thread::Builder::new()
        .name("swtrans-ipc".into())
        .spawn(move || {
            let addr = format!("127.0.0.1:{port}");
            let listener = match TcpListener::bind(&addr) {
                Ok(l) => l,
                Err(err) => {
                    eprintln!("swtrans: IPC bind {addr} failed: {err}");
                    return;
                }
            };
            let _ = listener.set_nonblocking(false);
            for stream in listener.incoming().flatten() {
                handle_client(stream, &tx);
            }
        })
        .ok();
}

fn handle_client(mut stream: TcpStream, tx: &Sender<AppCommand>) {
    let mut buf = [0u8; 1024];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let first = req.lines().next().unwrap_or("");
    let ok = if first.contains("/selection_translate") || first.contains("/translate-selection")
    {
        let _ = tx.send(AppCommand::TranslateSelection);
        true
    } else if first.contains("/settings") || first.contains("/config") {
        let _ = tx.send(AppCommand::OpenSettings);
        true
    } else {
        first.starts_with("GET / ") || first.starts_with("GET /HTTP")
    };

    let body = if ok { "ok" } else { "not found" };
    let status = if ok { "200 OK" } else { "404 Not Found" };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

pub fn trigger_settings() -> anyhow::Result<()> {
    let cfg = Config::load();
    let addr = format!("127.0.0.1:{}", cfg.ipc_port);
    let mut stream = TcpStream::connect(&addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(b"GET /settings HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")?;
    let mut _buf = Vec::new();
    let _ = stream.read_to_end(&mut _buf);
    Ok(())
}

pub fn trigger_translate() -> anyhow::Result<()> {
    let cfg = Config::load();
    let addr = format!("127.0.0.1:{}", cfg.ipc_port);
    let mut stream = TcpStream::connect(&addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(
        b"GET /selection_translate HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
    )?;
    let mut _buf = Vec::new();
    let _ = stream.read_to_end(&mut _buf);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn server_triggers_translate() {
        let (tx, rx) = mpsc::channel();
        start_server(18777, tx);
        thread::sleep(Duration::from_millis(80));
        let mut stream = TcpStream::connect("127.0.0.1:18777").expect("connect");
        stream
            .write_all(b"GET /selection_translate HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .unwrap();
        let cmd = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(cmd, AppCommand::TranslateSelection));
    }
}
