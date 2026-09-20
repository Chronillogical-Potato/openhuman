import { describe, expect, it } from 'vitest';

import { formatMcpConfig, mcpConfigChanged, parseMcpConfig } from './mcpJson';

describe('parseMcpConfig', () => {
  it('reads both spellings and strips the echoed fields', () => {
    const parsed = parseMcpConfig(
      JSON.stringify({
        mcpServers: {
          local: { command: 'npx', args: ['-y', 'x'], envKeys: ['TOKEN'], authConfigured: true },
          hosted: { url: 'https://h.test/mcp', headers: { Authorization: 'Bearer x' } },
        },
      })
    );
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;
    expect(parsed.doc.mcpServers.local).toEqual({ command: 'npx', args: ['-y', 'x'] });
    expect(parsed.doc.mcpServers.hosted).toEqual({
      url: 'https://h.test/mcp',
      headers: { Authorization: 'Bearer x' },
    });
  });

  it.each([
    ['', 'empty'],
    ['   \n', 'empty'],
    ['{', 'invalidJson'],
    ['[]', 'rootNotObject'],
    ['{}', 'missingRoot'],
    ['{ "mcpServers": 1 }', 'rootNotMap'],
    ['{ "mcpServers": { "": {} } }', 'emptyName'],
    ['{ "mcpServers": { "a": "x" } }', 'entryNotObject'],
    ['{ "mcpServers": { "a": {} } }', 'needsUrlOrCommand'],
    ['{ "mcpServers": { "a": { "url": "u", "command": "c" } } }', 'bothUrlAndCommand'],
  ])('refuses %j with a stable code', (text, code) => {
    const parsed = parseMcpConfig(text);
    expect(parsed.ok).toBe(false);
    if (parsed.ok) return;
    expect(parsed.code).toBe(code);
  });

  it('names the entry in a refusal', () => {
    const parsed = parseMcpConfig('{ "mcpServers": { "notion": {} } }');
    expect(parsed).toMatchObject({ ok: false, code: 'needsUrlOrCommand', name: 'notion' });
  });

  it('carries the JSON parser message as detail', () => {
    const parsed = parseMcpConfig('{ "mcpServers": ');
    expect(parsed).toMatchObject({ ok: false, code: 'invalidJson' });
    if (parsed.ok) return;
    expect(parsed.detail).toBeTruthy();
  });
});

describe('mcpConfigChanged', () => {
  const loaded = {
    mcpServers: {
      a: { command: 'npx', args: ['-y', 'a'], envKeys: ['K'], authConfigured: true },
    },
  };

  it('is false for the loaded document as formatted', () => {
    expect(mcpConfigChanged(formatMcpConfig(loaded), loaded)).toBe(false);
  });

  it('ignores whitespace and key order', () => {
    const reordered = '{"mcpServers":{"a":{"args":["-y","a"],"command":"npx"}}}';
    expect(mcpConfigChanged(reordered, loaded)).toBe(false);
  });

  it('ignores an edit to an echoed field', () => {
    const text = JSON.stringify({
      mcpServers: { a: { command: 'npx', args: ['-y', 'a'], authConfigured: false } },
    });
    expect(mcpConfigChanged(text, loaded)).toBe(false);
  });

  it('is true for a real edit, an unparsable buffer, and text before a load', () => {
    expect(
      mcpConfigChanged(JSON.stringify({ mcpServers: { a: { command: 'uvx' } } }), loaded)
    ).toBe(true);
    expect(mcpConfigChanged('{', loaded)).toBe(true);
    expect(mcpConfigChanged('{}', null)).toBe(true);
    expect(mcpConfigChanged('', null)).toBe(false);
  });
});
