import type { Conversation } from '../../src/models/Conversation';
import type { Message } from '../../src/models/Message';

describe('Conversation model', () => {
  function makeConversation(overrides: Partial<Conversation> = {}): Conversation {
    const msg: Message = {
      id: 'msg-1',
      from: 'alice@example.com',
      to: ['bob@example.com'],
      date: new Date().toISOString(),
      body: 'Hello',
      attachments: [],
      read: false,
    };
    return {
      id: 'conv-1',
      groupName: null,
      participants: ['alice@example.com', 'bob@example.com'],
      messages: [msg],
      lastMessage: msg,
      unreadCount: 1,
      ...overrides,
    };
  }

  it('represents a one-to-one conversation with null groupName', () => {
    const c = makeConversation();
    expect(c.groupName).toBeNull();
    expect(c.participants).toHaveLength(2);
  });

  it('represents a group conversation with a groupName', () => {
    const c = makeConversation({
      groupName: 'Team Chat',
      participants: ['alice@example.com', 'bob@example.com', 'carol@example.com'],
    });
    expect(c.groupName).toBe('Team Chat');
  });

  it('computes unreadCount correctly', () => {
    const c = makeConversation({ unreadCount: 3 });
    expect(c.unreadCount).toBe(3);
  });

  it('reflects the lastMessage', () => {
    const c = makeConversation();
    expect(c.lastMessage?.body).toBe('Hello');
  });
});
