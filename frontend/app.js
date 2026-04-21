/**
 * RoundChat — pure JavaScript front-end.
 *
 * Single responsibility per function.  Communicates with the Rust backend via
 * fetch() (JSON REST) and EventSource (SSE).  No frameworks or build tools.
 */

'use strict';

// ─── State ───────────────────────────────────────────────────────────────────

const state = {
  session: null,        // { email }
  conversations: [],    // Conversation[]
  activeConvo: null,    // Conversation | null
  contacts: [],         // Contact[]
  files: [],            // RemoteFile[]
  eventSource: null,    // EventSource | null
};

// ─── Bootstrap ───────────────────────────────────────────────────────────────

window.addEventListener('DOMContentLoaded', async () => {
  bindStaticEvents();
  const session = await api.getSession();
  if (session && session.logged_in) {
    state.session = session;
    showMain();
    await loadConversations();
    connectSSE();
  } else {
    showLogin();
  }
});

// ─── API helpers ─────────────────────────────────────────────────────────────

const api = {
  async request(method, path, body) {
    const opts = {
      method,
      headers: { 'Content-Type': 'application/json' },
    };
    if (body !== undefined) opts.body = JSON.stringify(body);
    const resp = await fetch(path, opts);
    if (!resp.ok) {
      const err = await resp.json().catch(() => ({ error: resp.statusText }));
      throw new Error(err.error || resp.statusText);
    }
    const text = await resp.text();
    return text ? JSON.parse(text) : null;
  },

  getSession()          { return this.request('GET',  '/api/session'); },
  login(email, pw)      { return this.request('POST', '/api/login',   { email, password: pw }); },
  logout()              { return this.request('POST', '/api/logout'); },
  getConversations()    { return this.request('GET',  '/api/conversations'); },
  sendMessage(to, body, group_name) {
    return this.request('POST', '/api/send', { to, body, group_name: group_name || null });
  },
  markRead(uid)         { return this.request('POST', `/api/mark-read/${uid}`); },
  getContacts()         { return this.request('GET',  '/api/contacts'); },
  getFiles()            { return this.request('GET',  '/api/files'); },
  deleteFile(url)       { return this.request('DELETE', '/api/files', { url }); },
};

// ─── Login ────────────────────────────────────────────────────────────────────

function bindStaticEvents() {
  // Login
  el('login-btn').addEventListener('click', handleLogin);
  el('login-password').addEventListener('keydown', e => {
    if (e.key === 'Enter') handleLogin();
  });

  // Sign out
  el('btn-signout').addEventListener('click', handleSignOut);

  // Tab switching
  document.querySelectorAll('.tab[data-pane]').forEach(btn => {
    btn.addEventListener('click', () => switchPane(btn.dataset.pane));
  });

  // Refresh buttons
  el('btn-refresh').addEventListener('click', () => loadConversations());
  el('btn-refresh-contacts').addEventListener('click', () => loadContacts());
  el('btn-refresh-files').addEventListener('click', () => loadFiles());

  // Back button in chat overlay
  el('btn-back').addEventListener('click', closeChat);

  // Send message
  el('chat-form').addEventListener('submit', handleSend);

  // File upload
  el('btn-upload').addEventListener('click', () => el('file-input').click());
  el('file-input').addEventListener('change', handleFileUpload);
}

async function handleLogin() {
  const email    = el('login-email').value.trim().toLowerCase();
  const password = el('login-password').value;
  const errorEl  = el('login-error');

  if (!email || !password) {
    showError(errorEl, 'Please enter your email and password.');
    return;
  }

  setLoginLoading(true);
  errorEl.classList.add('hidden');

  try {
    const session = await api.login(email, password);
    state.session = session;
    showMain();
    await loadConversations();
    connectSSE();
  } catch (err) {
    showError(errorEl, err.message || 'Login failed');
  } finally {
    setLoginLoading(false);
  }
}

function setLoginLoading(loading) {
  const btn = el('login-btn');
  btn.disabled = loading;
  btn.textContent = loading ? 'Signing in…' : 'Sign in';
}

async function handleSignOut() {
  try { await api.logout(); } catch (_) {}
  disconnectSSE();
  state.session = null;
  state.conversations = [];
  state.contacts = [];
  state.files = [];
  el('conversations-list').innerHTML = '<div class="placeholder">Loading…</div>';
  el('contacts-list').innerHTML     = '<div class="placeholder">Loading…</div>';
  el('files-list').innerHTML        = '<div class="placeholder">Loading…</div>';
  showLogin();
}

// ─── View switching ───────────────────────────────────────────────────────────

function showLogin() {
  el('view-login').classList.add('active');
  el('view-main').classList.remove('active');
}

function showMain() {
  el('view-login').classList.remove('active');
  el('view-main').classList.add('active');
}

function switchPane(paneId) {
  document.querySelectorAll('.tab[data-pane]').forEach(t =>
    t.classList.toggle('active', t.dataset.pane === paneId)
  );
  document.querySelectorAll('.pane').forEach(p =>
    p.classList.toggle('active', p.id === paneId)
  );
  // Lazy-load pane data.
  if (paneId === 'pane-contacts' && state.contacts.length === 0) loadContacts();
  if (paneId === 'pane-files'    && state.files.length === 0)    loadFiles();
}

// ─── Conversations ────────────────────────────────────────────────────────────

async function loadConversations() {
  setListContent('conversations-list', '<div class="placeholder">Loading…</div>');
  try {
    const convos = await api.getConversations();
    state.conversations = convos || [];
    renderConversations();
  } catch (err) {
    setListContent('conversations-list',
      `<div class="placeholder error-msg">${esc(err.message)}</div>`);
  }
}

function renderConversations() {
  const list = el('conversations-list');
  if (state.conversations.length === 0) {
    list.innerHTML = '<div class="placeholder">No conversations yet</div>';
    return;
  }
  list.innerHTML = state.conversations.map(convo => conversationRowHTML(convo)).join('');
  list.querySelectorAll('.convo-row').forEach((row, i) => {
    row.addEventListener('click', () => openChat(state.conversations[i]));
  });
}

function conversationRowHTML(convo) {
  const myEmail  = (state.session && state.session.email) || '';
  const others   = convo.participants.filter(p => p !== myEmail);
  const name     = convo.group_name || others.join(', ') || 'Unknown';
  const snippet  = convo.last_message ? esc(convo.last_message.body.slice(0, 80)) : '';
  const time     = convo.last_message ? fmtDate(convo.last_message.date) : '';
  const initials = avatarInitials(name);
  const badge    = convo.unread_count > 0
    ? `<span class="badge">${convo.unread_count}</span>` : '';

  return `
    <div class="convo-row">
      <div class="avatar">${initials}</div>
      <div class="convo-info">
        <div class="convo-name">${esc(name)}</div>
        <div class="convo-snippet">${snippet}</div>
      </div>
      <div class="convo-meta">
        <span class="convo-time">${esc(time)}</span>
        ${badge}
      </div>
    </div>`;
}

// ─── Chat detail ──────────────────────────────────────────────────────────────

function openChat(convo) {
  state.activeConvo = convo;
  const myEmail = (state.session && state.session.email) || '';
  const others  = convo.participants.filter(p => p !== myEmail);
  el('chat-title').textContent = convo.group_name || others.join(', ') || 'Chat';
  renderMessages(convo.messages);
  el('chat-overlay').classList.remove('hidden');
  el('chat-input').focus();
  scrollChatToBottom();
}

function closeChat() {
  state.activeConvo = null;
  el('chat-overlay').classList.add('hidden');
}

function renderMessages(messages) {
  const myEmail = (state.session && state.session.email) || '';
  const wrap = el('chat-messages');
  wrap.innerHTML = messages.map(m => messageBubbleHTML(m, myEmail)).join('');
  scrollChatToBottom();
}

function messageBubbleHTML(msg, myEmail) {
  const isMe     = msg.from === myEmail;
  const side     = isMe ? 'me' : 'them';
  const sender   = isMe ? 'You' : msg.from;
  const time     = fmtDate(msg.date);
  const bodyHtml = linkify(esc(msg.body));

  const senderLine = !isMe
    ? `<div class="bubble-sender">${esc(sender)}</div>`
    : '';

  const attachments = msg.attachments && msg.attachments.length > 0
    ? msg.attachments.map(a =>
        `<div>📎 <a href="${esc(a.url)}" target="_blank" rel="noreferrer">${esc(a.filename)}</a></div>`
      ).join('')
    : '';

  return `
    <div class="bubble-wrap ${side}">
      <div class="bubble">
        ${senderLine}
        <div>${bodyHtml}${attachments}</div>
        <div class="bubble-time">${esc(time)}</div>
      </div>
    </div>`;
}

async function handleSend(e) {
  e.preventDefault();
  const input = el('chat-input');
  const text  = input.value.trim();
  if (!text || !state.activeConvo) return;

  const convo   = state.activeConvo;
  const myEmail = (state.session && state.session.email) || '';
  const to      = convo.participants.filter(p => p !== myEmail);

  input.value   = '';
  input.disabled = true;

  try {
    await api.sendMessage(to, text, convo.group_name);
    // Optimistic UI: append bubble immediately.
    const optimistic = {
      id: `local-${Date.now()}`,
      from: myEmail,
      to,
      date: new Date().toISOString(),
      body: text,
      attachments: [],
      read: true,
    };
    convo.messages.push(optimistic);
    renderMessages(convo.messages);
  } catch (err) {
    alert('Could not send message: ' + err.message);
  } finally {
    input.disabled = false;
    input.focus();
  }
}

function scrollChatToBottom() {
  const el_ = el('chat-messages');
  el_.scrollTop = el_.scrollHeight;
}

// ─── Contacts ─────────────────────────────────────────────────────────────────

async function loadContacts() {
  setListContent('contacts-list', '<div class="placeholder">Loading…</div>');
  try {
    const contacts = await api.getContacts();
    state.contacts = contacts || [];
    renderContacts();
  } catch (err) {
    setListContent('contacts-list',
      `<div class="placeholder error-msg">${esc(err.message)}</div>`);
  }
}

function renderContacts() {
  const list = el('contacts-list');
  if (state.contacts.length === 0) {
    list.innerHTML = '<div class="placeholder">No contacts</div>';
    return;
  }
  list.innerHTML = state.contacts.map(c => `
    <div class="contact-row">
      <div class="avatar" style="background:${avatarColor(c.display_name)}">${avatarInitials(c.display_name)}</div>
      <div class="contact-info">
        <div class="contact-name">${esc(c.display_name)}</div>
        <div class="contact-email">${esc(c.email)}</div>
      </div>
    </div>`).join('');
}

// ─── Files ────────────────────────────────────────────────────────────────────

async function loadFiles() {
  setListContent('files-list', '<div class="placeholder">Loading…</div>');
  try {
    const files = await api.getFiles();
    state.files = files || [];
    renderFiles();
  } catch (err) {
    setListContent('files-list',
      `<div class="placeholder error-msg">${esc(err.message)}</div>`);
  }
}

function renderFiles() {
  const list = el('files-list');
  if (state.files.length === 0) {
    list.innerHTML = '<div class="placeholder">No files</div>';
    return;
  }
  list.innerHTML = state.files.map((f, i) => `
    <div class="file-row" data-idx="${i}">
      <div class="file-icon">${fileIcon(f.mime_type)}</div>
      <div class="file-info">
        <div class="file-name">${esc(f.name)}</div>
        <div class="file-meta">${fmtBytes(f.size)} · ${esc(f.last_modified)}</div>
      </div>
      <div class="file-actions">
        <a href="${esc(f.url)}" target="_blank" rel="noreferrer" download="${esc(f.name)}">Download</a>
        <button onclick="deleteFile(${i})">Delete</button>
      </div>
    </div>`).join('');
}

async function handleFileUpload() {
  const fileInput = el('file-input');
  const file = fileInput.files[0];
  if (!file) return;

  const formData = new FormData();
  formData.append('file', file, file.name);

  const btn = el('btn-upload');
  btn.disabled = true;
  btn.textContent = 'Uploading…';

  try {
    const resp = await fetch('/api/files', { method: 'POST', body: formData });
    if (!resp.ok) {
      const err = await resp.json().catch(() => ({ error: resp.statusText }));
      throw new Error(err.error);
    }
    await loadFiles();
  } catch (err) {
    alert('Upload failed: ' + err.message);
  } finally {
    btn.disabled = false;
    btn.textContent = 'Upload file';
    fileInput.value = '';
  }
}

async function deleteFile(idx) {
  const file = state.files[idx];
  if (!file) return;
  if (!confirm(`Delete "${file.name}"?`)) return;
  try {
    await api.deleteFile(file.url);
    await loadFiles();
  } catch (err) {
    alert('Delete failed: ' + err.message);
  }
}

// ─── SSE (real-time updates) ──────────────────────────────────────────────────

function connectSSE() {
  if (state.eventSource) return;
  const es = new EventSource('/api/events');
  es.addEventListener('refresh', () => {
    if (state.activeConvo) {
      // Re-fetch and update active conversation in place.
      api.getConversations().then(convos => {
        state.conversations = convos || [];
        const updated = convos.find(c => c.id === state.activeConvo.id);
        if (updated) {
          state.activeConvo = updated;
          renderMessages(updated.messages);
        }
        renderConversations();
      }).catch(() => {});
    } else {
      loadConversations();
    }
  });
  es.onerror = () => {
    // Reconnect after 5s on error.
    state.eventSource = null;
    setTimeout(connectSSE, 5000);
  };
  state.eventSource = es;
}

function disconnectSSE() {
  if (state.eventSource) {
    state.eventSource.close();
    state.eventSource = null;
  }
}

// ─── Utility helpers ──────────────────────────────────────────────────────────

function el(id) { return document.getElementById(id); }

function esc(str) {
  if (!str) return '';
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

/** Convert plain URLs in text to clickable links. */
function linkify(text) {
  return text.replace(
    /(https?:\/\/[^\s<>"]+)/g,
    url => `<a href="${url}" target="_blank" rel="noreferrer">${url}</a>`
  );
}

function fmtDate(isoStr) {
  if (!isoStr) return '';
  const d = new Date(isoStr);
  if (isNaN(d)) return isoStr;
  const now  = new Date();
  const diff = now - d;
  if (diff < 86400000 && d.getDate() === now.getDate()) {
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }
  if (diff < 7 * 86400000) {
    return d.toLocaleDateString([], { weekday: 'short' });
  }
  return d.toLocaleDateString([], { month: 'short', day: 'numeric' });
}

function fmtBytes(n) {
  if (n === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.min(Math.floor(Math.log2(n) / 10), units.length - 1);
  return `${(n / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

function avatarInitials(name) {
  if (!name) return '?';
  const parts = name.trim().split(/\s+/);
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
}

function avatarColor(name) {
  if (!name) return '#007AFF';
  const colors = [
    '#007AFF', '#5856D6', '#FF2D55', '#FF9500',
    '#34C759', '#00C7BE', '#AF52DE', '#FF3B30',
  ];
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = (hash * 31 + name.charCodeAt(i)) | 0;
  return colors[Math.abs(hash) % colors.length];
}

function fileIcon(mime) {
  if (!mime) return '📄';
  if (mime.startsWith('image/'))       return '🖼️';
  if (mime.startsWith('video/'))       return '🎬';
  if (mime.startsWith('audio/'))       return '🎵';
  if (mime.includes('pdf'))            return '📕';
  if (mime.includes('zip') || mime.includes('tar') || mime.includes('gz')) return '🗜️';
  if (mime.includes('word') || mime.includes('doc')) return '📝';
  if (mime.includes('sheet') || mime.includes('excel') || mime.includes('csv')) return '📊';
  return '📄';
}

function setListContent(id, html) {
  el(id).innerHTML = html;
}

function showError(errorEl, msg) {
  errorEl.textContent = msg;
  errorEl.classList.remove('hidden');
}
