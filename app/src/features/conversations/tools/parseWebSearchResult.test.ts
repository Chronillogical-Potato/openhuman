import { describe, expect, it } from 'vitest';

import { extractAgentSources } from '../../../utils/toolTimelineFormatting';
import { extractSearchProvider, parseWebSearchResult } from './parseWebSearchResult';

const TEXT = [
  'Search results for: rust async traits (via Exa)',
  '1. Async fn in traits are now stable',
  '   https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits.html',
  '   Published: 2023-12-21',
  '   Rust 1.75 stabilizes async fn in traits.',
  'This line wraps from the excerpt.',
  '2. javascript link',
  '   javascript:alert(1)',
  '3. Tokio tutorial',
  '   https://tokio.rs/tokio/tutorial',
  '   Learn async Rust.',
].join('\n');

describe('parseWebSearchResult', () => {
  it('parses the plain-text rendering every engine returns', () => {
    const parsed = parseWebSearchResult(TEXT);
    expect(parsed?.query).toBe('rust async traits');
    expect(parsed?.provider).toBe('Exa');
    expect(parsed?.results.map(r => r.domain)).toEqual(['blog.rust-lang.org', 'tokio.rs']);
    expect(parsed?.results[0]).toMatchObject({
      title: 'Async fn in traits are now stable',
      published: '2023-12-21',
      excerpt: 'Rust 1.75 stabilizes async fn in traits. This line wraps from the excerpt.',
    });
  });

  it('drops non-http(s) urls instead of rendering them as links', () => {
    const urls = parseWebSearchResult(TEXT)?.results.map(r => r.url) ?? [];
    expect(urls.every(url => url.startsWith('https://'))).toBe(true);
  });

  it('reports an empty search as empty, not unparseable', () => {
    expect(parseWebSearchResult('No results found for: zzqx (via Brave)')).toEqual({
      query: 'zzqx',
      provider: 'Brave',
      results: [],
      empty: true,
    });
  });

  it('keeps a "(via …)" inside the query out of the provider', () => {
    const parsed = parseWebSearchResult('Search results for: login (via OAuth) (via Exa)');
    expect(parsed?.provider).toBe('Exa');
    expect(parsed?.query).toBe('login (via OAuth)');
    expect(extractSearchProvider('Search results for: login (via OAuth) (via Exa)')).toBe('Exa');
  });

  it('parses the markdown rendering', () => {
    const md = [
      '# Search results — `vite plugins` (via Tavily)',
      '',
      '## [Vite plugin API](https://vite.dev/guide/api-plugin)',
      '_Published: 2025-01-01_',
      '',
      '> Plugins extend Vite.',
    ].join('\n');
    const parsed = parseWebSearchResult(md);
    expect(parsed?.provider).toBe('Tavily');
    expect(parsed?.results).toEqual([
      {
        title: 'Vite plugin API',
        url: 'https://vite.dev/guide/api-plugin',
        domain: 'vite.dev',
        published: '2025-01-01',
        excerpt: 'Plugins extend Vite.',
      },
    ]);
  });

  it('prefers the structured payload over the text', () => {
    const parsed = parseWebSearchResult('Search results for: ignored (via Exa)', {
      kind: 'web_search',
      query: 'structured',
      provider: 'Parallel',
      results: [{ title: 'A', url: 'https://www.a.dev/x', excerpt: 'e' }, { url: 'file:///etc' }],
    });
    expect(parsed).toEqual({
      query: 'structured',
      provider: 'Parallel',
      results: [{ title: 'A', url: 'https://www.a.dev/x', domain: 'a.dev', excerpt: 'e' }],
      empty: false,
    });
  });

  it('returns undefined for output it does not recognise', () => {
    expect(parseWebSearchResult('some other text')).toBeUndefined();
    expect(parseWebSearchResult(undefined)).toBeUndefined();
  });
});

describe('extractAgentSources', () => {
  it('lists the hits of a completed web search as sources', () => {
    const sources = extractAgentSources([
      {
        id: 's1',
        name: 'web_search_tool',
        round: 1,
        seq: 0,
        status: 'success',
        argsBuffer: '{"query":"rust async traits"}',
        result: TEXT,
      },
      {
        id: 'f1',
        name: 'web_fetch',
        round: 1,
        seq: 1,
        status: 'success',
        argsBuffer: '{"url":"https://tokio.rs/tokio/tutorial"}',
      },
    ]);
    expect(sources.map(s => s.url)).toEqual([
      'https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits.html',
      'https://tokio.rs/tokio/tutorial',
    ]);
    expect(sources[0].title).toBe('Async fn in traits are now stable');
  });
});
