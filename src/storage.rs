/// Small JSON-backed persistence layer for per-account chat history.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::models::MailCache;

fn account_key(email: &str) -> String {
    email
        .to_lowercase()
        .as_bytes()
        .iter()
        .fold(String::new(), |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        })
}

pub fn cache_path(data_dir: &str, email: &str) -> PathBuf {
    Path::new(data_dir)
        .join("mail")
        .join(format!("{}.json", account_key(email)))
}

pub fn load_mail_cache(data_dir: &str, email: &str) -> Result<MailCache> {
    let path = cache_path(data_dir, email);
    let bytes = std::fs::read(&path)
        .with_context(|| format!("read mail cache {}", path.display()))?;
    serde_json::from_slice(&bytes).context("decode mail cache")
}

/// Write only message data, never account credentials. A replace via a
/// temporary file avoids exposing a partially written JSON cache.
pub fn save_mail_cache(data_dir: &str, email: &str, cache: &MailCache) -> Result<()> {
    let path = cache_path(data_dir, email);
    let parent = path.parent().context("mail cache has no parent")?;
    std::fs::create_dir_all(parent).context("create mail cache directory")?;

    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec(cache).context("encode mail cache")?;
    let mut options = std::fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp).context("open temporary mail cache")?;
    file.write_all(&bytes).context("write temporary mail cache")?;
    file.sync_all().context("flush temporary mail cache")?;
    drop(file);

    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(&path).context("replace old mail cache")?;
    }
    std::fs::rename(&temp, &path).context("replace mail cache")?;
    Ok(())
}

/// RoundChat 0.1 persisted the mailbox password here. Remove that file on
/// startup; sessions are now represented only by random in-memory tokens.
pub fn remove_legacy_password_file(data_dir: &str) {
    let path = Path::new(data_dir).join("session.json");
    if path.exists() {
        if let Err(error) = std::fs::remove_file(&path) {
            tracing::warn!("Could not remove legacy plaintext session file: {error}");
        } else {
            tracing::warn!("Removed legacy session.json containing plaintext credentials");
        }
    }
}
