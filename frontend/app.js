/**
 * RoundChat — pure JavaScript front-end.
 *
 * Single responsibility per function.  Communicates with the Rust backend via
 * fetch() (JSON REST) and EventSource (SSE).  No frameworks or build tools.
 */

'use strict';

// ─── State ───────────────────────────────────────────────────────────────────

let attachObserver = null;

const state = {
  session: null,        // { email }
  conversations: [],    // Conversation[]
  activeConvo: null,    // Conversation | null
  contacts: [],         // Contact[]
  files: [],            // RemoteFile[]
  attachments: [],      // RemoteFile[] pending for current message
  attachGridChunk: 20,  // lazy-load chunk size
  eventSource: null,    // EventSource | null
};

// ─── Bootstrap ───────────────────────────────────────────────────────────────

window.addEventListener('DOMContentLoaded', async () => {
  bindStaticEvents();
  initTheme();
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

  // Tab switching
  document.querySelectorAll('.tab[data-pane]').forEach(btn => {
    btn.addEventListener('click', () => switchPane(btn.dataset.pane));
  });

  // Refresh buttons
  el('btn-refresh').addEventListener('click', () => loadConversations());
  el('btn-refresh-contacts').addEventListener('click', () => loadContacts());
  // Settings
  el('btn-settings-theme').addEventListener('click', toggleTheme);
  el('btn-settings-signout').addEventListener('click', handleSignOut);

  // Back button in chat overlay
  el('btn-back').addEventListener('click', closeChat);
  el('btn-chat-refresh').addEventListener('click', refreshChat);

  // Send message
  el('chat-form').addEventListener('submit', handleSend);

  // Attachment picker
  el('btn-attach').addEventListener('click', toggleAttachPicker);
  el('btn-close-attach').addEventListener('click', closeAttachPicker);
  el('btn-upload-attach').addEventListener('click', () => el('attach-file-input').click());
  el('attach-file-input').addEventListener('change', handleAttachUpload);
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
  state.attachments = [];
  el('conversations-list').innerHTML = '<div class="placeholder">Loading…</div>';
  el('contacts-list').innerHTML     = '<div class="placeholder">Loading…</div>';
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
  if (!state.activeConvo) return;
  if (!text && (!state.attachments || state.attachments.length === 0)) return;

  const convo   = state.activeConvo;
  const myEmail = (state.session && state.session.email) || '';
  const to      = convo.participants.filter(p => p !== myEmail);

  if (to.length === 0) {
    alert('No other participants in this conversation to send to.');
    return;
  }

  const attachRefs = (state.attachments || []).map(f =>
    `📎 ${f.name} (${f.url})`
  ).join('\n');
  const fullBody = attachRefs ? (text ? attachRefs + '\n\n' + text : attachRefs) : text;

  input.value   = '';
  input.disabled = true;

  try {
    await api.sendMessage(to, fullBody, convo.group_name);
    const optimistic = {
      id: `local-${Date.now()}`,
      from: myEmail,
      to,
      date: new Date().toISOString(),
      body: fullBody,
      attachments: (state.attachments || []).map(f => ({
        url: f.url,
        mime_type: f.mime_type,
        filename: f.name,
        size: 0,
      })),
      read: true,
    };
    convo.messages.push(optimistic);
    renderMessages(convo.messages);
    state.attachments = [];
    renderAttachChips();
    closeAttachPicker();
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

async function refreshChat() {
  if (!state.activeConvo) return;
  const activeParticipants = [...state.activeConvo.participants].sort();
  const activeGroupName = state.activeConvo.group_name;
  await loadConversations();
  const refreshed = state.conversations.find(c =>
    c.group_name === activeGroupName &&
    [...c.participants].sort().join() === activeParticipants.join()
  );
  if (refreshed) {
    state.activeConvo = refreshed;
    renderMessages(refreshed.messages);
  }
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

// ─── Files / Attachment Picker ────────────────────────────────────────────────

async function loadFiles() {
  try {
    const files = await api.getFiles();
    state.files = files || [];
  } catch (_) {
    state.files = [];
  }
}

function renderAttachGrid() {
  const grid = el('attach-grid');
  const files = state.files;
  if (!files || files.length === 0) {
    grid.innerHTML = '<div class="placeholder">No files yet</div>';
    return;
  }
  const chunk = state.attachGridChunk || 20;
  const shown = files.slice(0, chunk);
  grid.innerHTML = shown.map((f, i) => attachGridItemHTML(f, i)).join('');

  const sentinel = document.createElement('div');
  sentinel.className = 'attach-grid-sentinel';
  grid.appendChild(sentinel);

  if (chunk >= files.length) return;

  if (attachObserver) attachObserver.disconnect();
  attachObserver = new IntersectionObserver(entries => {
    if (entries[0].isIntersecting) {
      state.attachGridChunk = (state.attachGridChunk || 20) + 20;
      renderAttachGrid();
    }
  }, { root: grid, rootMargin: '150px' });
  attachObserver.observe(sentinel);
}

function attachGridItemHTML(file, idx) {
  const isImage = file.mime_type && file.mime_type.startsWith('image/');
  const isSelected = state.attachments && state.attachments.some(a => a.url === file.url);
  return `
    <div class="attach-grid-item ${isSelected ? 'selected' : ''}" data-idx="${idx}">
      ${isImage
        ? `<div class="attach-thumb" style="background-image:url(${esc(file.url)})"></div>`
        : `<div class="attach-icon-large">${fileIcon(file.mime_type)}</div>`
      }
      <div class="attach-item-name">${esc(file.name)}</div>
    </div>`;
}

async function toggleAttachPicker() {
  if (el('attach-picker').classList.contains('hidden')) {
    openAttachPicker();
  } else {
    closeAttachPicker();
  }
}

async function openAttachPicker() {
  el('attach-picker').classList.remove('hidden');
  state.attachGridChunk = 20;
  await loadFiles();
  renderAttachGrid();
  el('attach-grid').addEventListener('click', onAttachGridClick);
}

function closeAttachPicker() {
  el('attach-picker').classList.add('hidden');
  if (attachObserver) attachObserver.disconnect();
}

function onAttachGridClick(e) {
  const item = e.target.closest('.attach-grid-item');
  if (!item) return;
  const idx = parseInt(item.dataset.idx);
  const file = state.files[idx];
  if (!file) return;
  toggleAttachment(file);
}

function toggleAttachment(file) {
  if (!state.attachments) state.attachments = [];
  const idx = state.attachments.findIndex(a => a.url === file.url);
  if (idx > -1) {
    state.attachments.splice(idx, 1);
  } else {
    state.attachments.push(file);
  }
  renderAttachGrid();
  renderAttachChips();
}

function renderAttachChips() {
  const container = el('attach-chips');
  const files = state.attachments || [];
  if (files.length === 0) {
    container.classList.add('hidden');
    container.innerHTML = '';
    return;
  }
  container.classList.remove('hidden');
  container.innerHTML = files.map((f, i) => `
    <span class="attach-chip">
      ${fileIcon(f.mime_type)} ${esc(f.name)}
      <button class="chip-remove" data-idx="${i}" type="button">✕</button>
    </span>`).join('');
  container.querySelectorAll('.chip-remove').forEach(btn => {
    btn.addEventListener('click', e => {
      e.stopPropagation();
      const idx = parseInt(btn.dataset.idx);
      state.attachments.splice(idx, 1);
      renderAttachChips();
      if (!el('attach-picker').classList.contains('hidden')) renderAttachGrid();
    });
  });
}

async function handleAttachUpload() {
  const input = el('attach-file-input');
  const file = input.files[0];
  if (!file) return;

  const formData = new FormData();
  formData.append('file', file, file.name);

  const btn = el('btn-upload-attach');
  btn.disabled = true;
  btn.textContent = 'Uploading…';

  try {
    const resp = await fetch('/api/files', { method: 'POST', body: formData });
    if (!resp.ok) {
      const err = await resp.json().catch(() => ({ error: resp.statusText }));
      throw new Error(err.error);
    }
    await loadFiles();
    renderAttachGrid();
  } catch (err) {
    alert('Upload failed: ' + err.message);
  } finally {
    btn.disabled = false;
    btn.textContent = '+ Upload file';
    input.value = '';
  }
}

// ─── SSE (real-time updates) ──────────────────────────────────────────────────

function connectSSE() {
  if (state.eventSource) return;
  const es = new EventSource('/api/events');
  es.onerror = () => {
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

// ─── Dark mode ────────────────────────────────────────────────────────────────

function initTheme() {
  const saved = localStorage.getItem('roundchat-theme');
  if (saved) {
    applyTheme(saved);
  } else if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
    applyTheme('dark');
  } else {
    applyTheme('light');
  }
}

function toggleTheme() {
  const current = document.documentElement.getAttribute('data-theme') || 'light';
  applyTheme(current === 'dark' ? 'light' : 'dark');
}

function applyTheme(theme) {
  document.documentElement.setAttribute('data-theme', theme);
  localStorage.setItem('roundchat-theme', theme);
  const isDark = theme === 'dark';
  el('settings-theme-icon').textContent = isDark ? '☀️' : '🌙';
  el('settings-theme-label').textContent = isDark ? 'Light mode' : 'Dark mode';
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
