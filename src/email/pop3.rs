/// POP3 client.
///
/// A minimal synchronous POP3 client using native-tls.  Wraps in
/// `tokio::task::spawn_blocking` for async compatibility.

use anyhow::{anyhow, Context, Result};
use native_tls::{TlsConnector, TlsStream};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use crate::config::Pop3Config;
use crate::email::parser::parse_raw_message;
use crate::models::Message;

enum Pop3Stream {
    Plain(BufReader<TcpStream>),
    Tls(BufReader<TlsStream<TcpStream>>),
}

impl Pop3Stream {
    fn read_line(&mut self) -> Result<String> {
        let mut line = String::new();
        match self {
            Pop3Stream::Plain(r) => { r.read_line(&mut line)?; }
            Pop3Stream::Tls(r) => { r.read_line(&mut line)?; }
        }
        Ok(line.trim_end_matches("\r\n").to_string())
    }

    fn write_all(&mut self, data: &[u8]) -> Result<()> {
        match self {
            Pop3Stream::Plain(r) => r.get_mut().write_all(data)?,
            Pop3Stream::Tls(r) => r.get_mut().write_all(data)?,
        }
        Ok(())
    }

    fn read_multiline(&mut self) -> Result<String> {
        let mut body = String::new();
        loop {
            let line = self.read_line()?;
            if line == "." {
                break;
            }
            // Dot-stuffing: lines starting with ".." become "."
            let line = if line.starts_with("..") { &line[1..] } else { &line };
            body.push_str(line);
            body.push_str("\r\n");
        }
        Ok(body)
    }
}

/// Connect to the POP3 server and authenticate.
fn connect(config: &Pop3Config, email: &str, password: &str) -> Result<Pop3Stream> {
    let addr = format!("{}:{}", config.host, config.port);
    let tcp = TcpStream::connect(&addr).context("POP3 TCP connect failed")?;
    tcp.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;

    let mut stream = if config.tls {
        let connector = TlsConnector::builder()
            .build()
            .context("TLS build failed")?;
        let tls_stream = connector
            .connect(config.host.as_str(), tcp)
            .context("POP3 TLS handshake failed")?;
        Pop3Stream::Tls(BufReader::new(tls_stream))
    } else {
        Pop3Stream::Plain(BufReader::new(tcp))
    };

    // Read greeting.
    let greeting = stream.read_line()?;
    if !greeting.starts_with("+OK") {
        return Err(anyhow!("POP3 unexpected greeting: {}", greeting));
    }

    // USER
    stream.write_all(format!("USER {email}\r\n").as_bytes())?;
    let resp = stream.read_line()?;
    if !resp.starts_with("+OK") {
        return Err(anyhow!("POP3 USER failed: {}", resp));
    }

    // PASS
    stream.write_all(format!("PASS {password}\r\n").as_bytes())?;
    let resp = stream.read_line()?;
    if !resp.starts_with("+OK") {
        return Err(anyhow!("POP3 authentication failed: {}", resp));
    }

    Ok(stream)
}

/// Fetch all messages via POP3.
pub async fn fetch_messages(
    config: Pop3Config,
    email: String,
    password: String,
) -> Result<(Vec<Message>, HashMap<String, String>)> {
    tokio::task::spawn_blocking(move || {
        let mut stream = connect(&config, &email, &password)?;

        // STAT to get message count.
        stream.write_all(b"STAT\r\n")?;
        let stat = stream.read_line()?;
        if !stat.starts_with("+OK") {
            return Err(anyhow!("POP3 STAT failed: {}", stat));
        }
        let parts: Vec<&str> = stat.split_whitespace().collect();
        let count: usize = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);

        let mut messages = Vec::new();
        let subject_map = HashMap::new(); // POP3 has no server-side threading

        for i in 1..=count {
            stream.write_all(format!("RETR {i}\r\n").as_bytes())?;
            let ok = stream.read_line()?;
            if !ok.starts_with("+OK") {
                tracing::warn!("POP3 RETR {i} failed: {ok}");
                continue;
            }
            match stream.read_multiline() {
                Ok(raw) => {
                    match parse_raw_message(&raw, &i.to_string()) {
                        Ok(msg) => messages.push(msg),
                        Err(e) => tracing::warn!("POP3 parse msg {i}: {e}"),
                    }
                }
                Err(e) => tracing::warn!("POP3 read multiline {i}: {e}"),
            }
        }

        // QUIT
        stream.write_all(b"QUIT\r\n").ok();

        Ok((messages, subject_map))
    })
    .await
    .context("spawn_blocking panicked")?
}
