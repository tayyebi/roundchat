import { buildConversations } from '../../src/services/ConversationBuilder';
import type { Message } from '../../src/models/Message';

function makeMessage(overrides: Partial<Message>): Message {
  return {
    id: 'msg-1',
    from: 'alice@example.com',
    to: ['bob@example.com'],
    date: new Date().toISOString(),
    body: 'Hello',
    attachments: [],
    read: false,
    ...overrides,
  };
}

describe('ConversationBuilder', () => {
  const localEmail = 'alice@example.com';

  it('groups messages with the same participants into one conversation', () => {
    const messages: Message[] = [
      makeMessage({ id: 'msg-1', from: 'alice@example.com', to: ['bob@example.com'] }),
      makeMessage({ id: 'msg-2', from: 'bob@example.com', to: ['alice@example.com'] }),
    ];
    const convos = buildConversations(messages, localEmail, new Map());
    expect(convos).toHaveLength(1);
  });

  it('creates separate conversations for different participant sets', () => {
    const messages: Message[] = [
      makeMessage({ id: 'msg-1', from: 'alice@example.com', to: ['bob@example.com'] }),
      makeMessage({ id: 'msg-2', from: 'alice@example.com', to: ['carol@example.com'] }),
    ];
    const convos = buildConversations(messages, localEmail, new Map());
    expect(convos).toHaveLength(2);
  });

  it('sorts messages oldest-first within a conversation', () => {
    const old = makeMessage({ id: 'old', date: '2024-01-01T10:00:00.000Z', from: 'alice@example.com', to: ['bob@example.com'] });
    const recent = makeMessage({ id: 'new', date: '2024-01-02T10:00:00.000Z', from: 'alice@example.com', to: ['bob@example.com'] });
    const convos = buildConversations([recent, old], localEmail, new Map());
    expect(convos[0].messages[0].id).toBe('old');
    expect(convos[0].messages[1].id).toBe('new');
  });

  it('sorts conversations most-recent-first', () => {
    const msgA = makeMessage({ id: 'a', date: '2024-01-01T00:00:00.000Z', from: 'alice@example.com', to: ['bob@example.com'] });
    const msgB = makeMessage({ id: 'b', date: '2024-01-02T00:00:00.000Z', from: 'alice@example.com', to: ['carol@example.com'] });
    const convos = buildConversations([msgA, msgB], localEmail, new Map());
    expect(convos[0].lastMessage?.id).toBe('b');
  });

  it('counts unread messages correctly', () => {
    const messages: Message[] = [
      makeMessage({ id: 'r', read: true, from: 'alice@example.com', to: ['bob@example.com'] }),
      makeMessage({ id: 'u', read: false, from: 'bob@example.com', to: ['alice@example.com'] }),
    ];
    const convos = buildConversations(messages, localEmail, new Map());
    expect(convos[0].unreadCount).toBe(1);
  });

  it('uses subject map for group name when multiple participants', () => {
    const msg = makeMessage({
      id: 'g1',
      from: 'alice@example.com',
      to: ['bob@example.com', 'carol@example.com'],
    });
    const subjectMap = new Map([['g1', 'Team Chat']]);
    const convos = buildConversations([msg], localEmail, subjectMap);
    expect(convos[0].groupName).toBe('Team Chat');
  });

  it('sets lastMessage to the most recent message in the thread', () => {
    const m1 = makeMessage({ id: 'm1', date: '2024-01-01T08:00:00.000Z', from: 'alice@example.com', to: ['bob@example.com'] });
    const m2 = makeMessage({ id: 'm2', date: '2024-01-01T09:00:00.000Z', from: 'bob@example.com', to: ['alice@example.com'] });
    const convos = buildConversations([m1, m2], localEmail, new Map());
    expect(convos[0].lastMessage?.id).toBe('m2');
  });

  it('handles empty message list', () => {
    expect(buildConversations([], localEmail, new Map())).toEqual([]);
  });
});
