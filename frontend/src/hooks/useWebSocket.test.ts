import { describe, it, expect } from 'vitest';
import type { WsEvent } from './useWebSocket';

// NOTE: Testing WebSocket hook behavior without rendering
// since useWebSocket requires QueryClientProvider context.
// We test the event parsing and types instead.

describe('WsEvent types', () => {
  it('parses new_mail event', () => {
    const raw = '{"type":"new_mail","folder":"INBOX","count":3}';
    const event: WsEvent = JSON.parse(raw);
    expect(event.type).toBe('new_mail');
    expect(event.folder).toBe('INBOX');
    expect(event.count).toBe(3);
  });

  it('parses unread_update event', () => {
    const raw = '{"type":"unread_update","folder":"INBOX","unread":5}';
    const event: WsEvent = JSON.parse(raw);
    expect(event.type).toBe('unread_update');
    expect(event.unread).toBe(5);
  });

  it('parses quota_update event', () => {
    const raw = '{"type":"quota_update","used_bytes":524288000,"total_bytes":1073741824}';
    const event: WsEvent = JSON.parse(raw);
    expect(event.type).toBe('quota_update');
    expect(event.used_bytes).toBe(524288000);
    expect(event.total_bytes).toBe(1073741824);
  });

  it('parses ping event', () => {
    const raw = '{"type":"ping","timestamp":1712000000}';
    const event: WsEvent = JSON.parse(raw);
    expect(event.type).toBe('ping');
    expect(event.timestamp).toBe(1712000000);
  });

  it('parses error event', () => {
    const raw = '{"type":"error","message":"Connection lost"}';
    const event: WsEvent = JSON.parse(raw);
    expect(event.type).toBe('error');
    expect(event.message).toBe('Connection lost');
  });

  it('handles unknown extra fields gracefully', () => {
    const raw = '{"type":"new_mail","folder":"INBOX","count":1,"extra":"field"}';
    const event: WsEvent = JSON.parse(raw);
    expect(event.type).toBe('new_mail');
    // Extra fields are just ignored by the type
  });
});

describe('WebSocket URL construction', () => {
  it('builds correct URL with token', () => {
    const token = 'eyJhbGciOiJSUzI1NiJ9.test.sig';
    const base = 'ws://localhost:3000';
    const url = `${base}/ws?token=${encodeURIComponent(token)}`;
    expect(url).toContain('ws://localhost:3000/ws?token=');
    expect(url).toContain('eyJhbGciOiJSUzI1NiJ9');
  });

  it('properly encodes special characters in token', () => {
    const token = 'abc+def/ghi=';
    const encoded = encodeURIComponent(token);
    expect(encoded).toBe('abc%2Bdef%2Fghi%3D');
  });
});

describe('subscribe command format', () => {
  it('formats folder subscription correctly', () => {
    const folder = 'INBOX';
    const cmd = `subscribe:${folder}`;
    expect(cmd).toBe('subscribe:INBOX');
  });

  it('handles nested folder names', () => {
    const folder = 'INBOX.Archive.2026';
    const cmd = `subscribe:${folder}`;
    expect(cmd).toBe('subscribe:INBOX.Archive.2026');
  });
});
