/// Configuration module.
///
/// All server settings are read from environment variables.  This is the
/// single source of truth for every host, port, and toggle in the app.
/// Nothing outside this module reads `std::env::var` directly.

#[derive(Debug, Clone)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub tls_insecure: bool,
}

#[derive(Debug, Clone)]
pub struct Pop3Config {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub tls_insecure: bool,
    pub poll_interval_secs: u64,
}

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub tls: bool,
}

#[derive(Debug, Clone)]
pub struct CardDavConfig {
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct WebDavConfig {
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub imap: Option<ImapConfig>,
    pub pop3: Option<Pop3Config>,
    pub smtp: SmtpConfig,
    pub carddav: Option<CardDavConfig>,
    pub webdav: Option<WebDavConfig>,
    /// Directory for persistent data (session, cache). Default: /data/roundchat
    pub data_dir: String,
    /// IP address the HTTP server binds to (env: ROUNDCHAT_HOST, default: 127.0.0.1).
    pub host: String,
    /// TCP port the HTTP server listens on (env: ROUNDCHAT_PORT, default: 7979).
    pub port: u16,
}

fn strip_inline_comment(value: String) -> String {
    // systemd EnvironmentFile does not strip inline shell-style comments.
    // Trim everything from the first " #" or "\t#" onward.
    let pos = value.find(" #").or_else(|| value.find("\t#"));
    match pos {
        Some(p) => value[..p].trim_end().to_string(),
        None => value,
    }
}

fn env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(strip_inline_comment)
        .filter(|v| !v.is_empty())
}

fn env_or(key: &str, default: &str) -> String {
    env(key).unwrap_or_else(|| default.to_string())
}

fn env_bool(key: &str, default: bool) -> bool {
    match env(key).as_deref() {
        Some("true") | Some("1") | Some("yes") => true,
        Some("false") | Some("0") | Some("no") => false,
        _ => default,
    }
}

fn env_u16(key: &str, default: u16) -> u16 {
    env(key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    env(key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

impl AppConfig {
    /// Load config from environment variables.
    /// IMAP is preferred; if IMAP_HOST is absent the app falls back to POP3.
    pub fn from_env() -> Self {
        let imap = env("IMAP_HOST").map(|host| ImapConfig {
            host,
            port: env_u16("IMAP_PORT", 993),
            tls: env_bool("IMAP_TLS", true),
            tls_insecure: env_bool("IMAP_TLS_INSECURE", false),
        });

        let pop3 = if imap.is_none() {
            env("POP3_HOST").map(|host| Pop3Config {
                host,
                port: env_u16("POP3_PORT", 995),
                tls: env_bool("POP3_TLS", true),
                tls_insecure: env_bool("POP3_TLS_INSECURE", false),
                poll_interval_secs: env_u64("POP3_POLL_INTERVAL", 30),
            })
        } else {
            None
        };

        let smtp = SmtpConfig {
            host: env_or("SMTP_HOST", "localhost"),
            port: env_u16("SMTP_PORT", 465),
            tls: env_bool("SMTP_TLS", true),
        };

        let carddav = env("CARDDAV_URL").map(|url| CardDavConfig { url });
        let webdav = env("WEBDAV_URL").map(|url| WebDavConfig { url });
        let data_dir = env_or("ROUNDCHAT_DATA", "/data/roundchat");
        let host = env_or("ROUNDCHAT_HOST", "127.0.0.1");
        let port = env_u16("ROUNDCHAT_PORT", 7979);

        AppConfig { imap, pop3, smtp, carddav, webdav, data_dir, host, port }
    }
}
