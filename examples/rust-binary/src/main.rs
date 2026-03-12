use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::Instant;

fn main() {
    let addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1:9090".to_string());
    let listener = TcpListener::bind(&addr).expect("failed to bind");
    let start = Instant::now();

    println!("Listening on {addr} (PID: {})", std::process::id());

    // Velos sends SIGTERM on stop/restart — the default handler will
    // terminate the process. For graceful shutdown in production,
    // use the `signal-hook` or `tokio::signal` crate.

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Accept error: {e}");
                continue;
            }
        };

        let mut request_line = String::new();
        let _ = BufReader::new(&stream).read_line(&mut request_line);

        let body = if request_line.starts_with("GET /health") {
            let uptime = start.elapsed().as_secs();
            format!("{{\"status\":\"ok\",\"uptime\":{uptime}}}")
        } else {
            format!("Hello from Velos! PID: {}\n", std::process::id())
        };

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
    }
}
