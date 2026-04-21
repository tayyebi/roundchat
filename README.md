# roundchat

Chat built on the universal language of the internet: email.

A desktop chat application that presents email as a modern messenger experience.
Built entirely on standard protocols (IMAP, POP3, SMTP, CardDAV, WebDAV) with
no proprietary backend — just a clever reinterpretation of email.

## Stack

| Layer    | Technology |
|----------|-----------|
| Backend  | Rust (axum, tokio) |
| Frontend | HTML5 + CSS3 + pure JS (embedded in the binary) |
| Protocols | IMAP / POP3 for incoming, SMTP for outgoing, CardDAV contacts, WebDAV files |

## Quick start

```sh
# Copy and edit the config
cp .env.example .env
$EDITOR .env

# Run (debug)
cargo run

# Or run (release — opens browser automatically)
cargo run --release
```

The binary starts a local HTTP server on `http://127.0.0.1:7979` (configurable
via `ROUNDCHAT_PORT`) and opens your default browser automatically.

## Configuration

All settings are read from environment variables (see `.env.example`):

| Variable | Description | Default |
|---|---|---|
| `IMAP_HOST` | IMAP server hostname — set this to use IMAP (preferred) | — |
| `IMAP_PORT` | IMAP port | `993` |
| `IMAP_TLS` | Enable TLS | `true` |
| `POP3_HOST` | POP3 server — used when `IMAP_HOST` is unset | — |
| `POP3_PORT` | POP3 port | `995` |
| `POP3_TLS` | Enable TLS | `true` |
| `POP3_POLL_INTERVAL` | Polling interval in seconds | `30` |
| `SMTP_HOST` | SMTP server hostname | `localhost` |
| `SMTP_PORT` | SMTP port | `465` |
| `SMTP_TLS` | Enable TLS | `true` |
| `CARDDAV_URL` | CardDAV address book URL (`{email}` placeholder) | — |
| `WEBDAV_URL` | WebDAV file storage URL (`{email}` placeholder) | — |
| `ROUNDCHAT_PORT` | Local HTTP server port | `7979` |

## Building release binaries

```sh
# Linux x86-64
cargo build --release --target x86_64-unknown-linux-gnu

# Windows x86-64 (cross-compile or run on Windows)
cargo build --release --target x86_64-pc-windows-msvc
```

GitHub Actions builds both targets automatically on every push (see
`.github/workflows/build.yml`). Artifacts are uploaded and available for
download from the Actions tab.

## Architecture

```
src/
├── main.rs              # Starts HTTP server, opens browser
├── config.rs            # Environment-variable config
├── models.rs            # Message, Conversation, Contact, RemoteFile, Session
├── state.rs             # Shared AppState (Arc<RwLock<...>> + SSE channel)
├── email/
│   ├── parser.rs        # RFC 5322 → Message (mailparse)
│   ├── conversation.rs  # Group messages into Conversation threads
│   ├── imap.rs          # IMAP client (imap crate, spawn_blocking)
│   ├── pop3.rs          # POP3 client (sync TCP+TLS, spawn_blocking)
│   ├── smtp.rs          # SMTP send (lettre, async)
│   └── mail_service.rs  # Orchestrator — auto-selects IMAP vs POP3
├── dav/
│   ├── carddav.rs       # CardDAV PROPFIND + vCard → Contact
│   └── webdav.rs        # WebDAV list / upload / delete
└── api/
    ├── mod.rs           # Axum router + embedded static files
    ├── auth.rs          # POST /api/login, POST /api/logout, GET /api/session
    ├── conversations.rs # GET /api/conversations, POST /api/send
    ├── contacts.rs      # GET /api/contacts
    ├── files.rs         # GET/POST/DELETE /api/files
    └── sse.rs           # GET /api/events (Server-Sent Events)

frontend/
├── index.html           # App shell (login + main views)
├── style.css            # Messenger-style CSS3 design
└── app.js               # All UI logic (pure JS, no frameworks)
```
