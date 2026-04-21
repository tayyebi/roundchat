# RoundChat

**Your email inbox, reimagined as a modern messenger.**

RoundChat is a desktop chat app that works directly with your existing email account — no new account to create, no proprietary server, no data stored anywhere except your own inbox. Open it in your browser and chat the way you already do, just with a familiar messenger interface.

---

## Screenshots

| Sign in | Conversations | Chat |
|---------|---------------|------|
| ![Sign-in screen](https://placehold.co/280x480/1c1c1e/ffffff?text=Sign+In) | ![Conversation list](https://placehold.co/280x480/1c1c1e/ffffff?text=Conversations) | ![Chat bubbles](https://placehold.co/280x480/1c1c1e/ffffff?text=Chat) |

| Contacts | Files |
|----------|-------|
| ![Contacts list](https://placehold.co/280x480/1c1c1e/ffffff?text=Contacts) | ![File manager](https://placehold.co/280x480/1c1c1e/ffffff?text=Files) |

---

## What you get

- **💬 Conversations** — your email threads displayed as chat bubbles, grouped by contact or group
- **✉️ Send & receive** — reply directly from the chat view; messages are real emails under the hood
- **👤 Contacts** — browse your address book pulled from CardDAV
- **📁 Files** — upload, download, and delete files stored on your WebDAV server
- **🔔 Real-time updates** — new messages appear automatically without refreshing

---

## Getting started

### 1 · Download

Grab the latest pre-built binary for your platform from the
[**Actions → Artifacts**](../../actions) tab (Linux x86-64 or Windows x86-64).

### 2 · Configure

Copy the example configuration file and fill in your email server details:

```sh
cp .env.example .env
```

Open `.env` in any text editor and set at minimum:

| Setting | What to put here |
|---------|-----------------|
| `IMAP_HOST` | Your mail server (e.g. `imap.gmail.com`) |
| `SMTP_HOST` | Your outgoing server (e.g. `smtp.gmail.com`) |

See `.env.example` for the full list of options including POP3, CardDAV and WebDAV.

### 3 · Run

```sh
./roundchat          # Linux
roundchat.exe        # Windows
```

RoundChat opens your default browser at `http://127.0.0.1:7979` automatically.
Sign in with your email address and password (or app-specific password).

---

## Privacy & security

- **No cloud, no accounts** — RoundChat runs entirely on your machine and talks directly to your own mail/DAV servers.
- **No data leaves your control** — messages are your normal emails; contacts and files live on your own server.
- **TLS on by default** — all connections to mail and DAV servers use TLS.

---

<details>
<summary>Developer notes</summary>

### Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust (axum, tokio) |
| Frontend | HTML5 + CSS3 + pure JS (embedded in the binary) |
| Protocols | IMAP / POP3, SMTP, CardDAV, WebDAV |

### Build from source

```sh
# Debug run
cargo run

# Release (opens browser automatically)
cargo run --release

# Cross-compile
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-pc-windows-msvc
```

GitHub Actions builds both targets on every push and uploads artifacts.

### Configuration reference

| Variable | Description | Default |
|---|---|---|
| `IMAP_HOST` | IMAP server hostname | — |
| `IMAP_PORT` | IMAP port | `993` |
| `IMAP_TLS` | Enable TLS | `true` |
| `POP3_HOST` | POP3 server (used when `IMAP_HOST` is unset) | — |
| `POP3_PORT` | POP3 port | `995` |
| `POP3_TLS` | Enable TLS | `true` |
| `POP3_POLL_INTERVAL` | Polling interval in seconds | `30` |
| `SMTP_HOST` | SMTP server hostname | `localhost` |
| `SMTP_PORT` | SMTP port | `465` |
| `SMTP_TLS` | Enable TLS | `true` |
| `CARDDAV_URL` | CardDAV address book URL (`{email}` placeholder) | — |
| `WEBDAV_URL` | WebDAV file storage URL (`{email}` placeholder) | — |
| `ROUNDCHAT_PORT` | Local HTTP server port | `7979` |

</details>
