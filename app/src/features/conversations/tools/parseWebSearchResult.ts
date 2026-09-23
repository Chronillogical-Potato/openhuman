/**
 * Turn a web-search tool result into rows the search element can render.
 *
 * Three inputs, most trustworthy first:
 *
 * 1. The structured payload a current core attaches to `tool_result`
 *    (`{ kind: "web_search", query, provider, results: [...] }`).
 * 2. The plain-text rendering every engine returns to the model:
 *
 *        Search results for: <query> (via <Provider>)
 *        1. <title>
 *           <url>
 *           Published: <date>
 *           <excerpt>
 *
 * 3. The markdown rendering (`## [title](url)` / `> excerpt`) used when the
 *    core prefers markdown.
 *
 * Every URL is model- or provider-supplied, so only well-formed `http(s)`
 * URLs are admitted; anything else is dropped rather than rendered as a
 * link.
 */
/** Upper bound on a provider label, so a malformed marker can't blow up a row. */
const MAX_SEARCH_PROVIDER_LENGTH = 32;

/**
 * Extract the resolved search provider from a completed web-search result.
 * Every search engine tags its output with a `(via <Provider>)` marker on the
 * heading line (managed resolves to "Exa" by default, or to whatever the
 * backend reports; BYOK engines tag "Brave"/"Querit"/"Seltz"/"Tavily"). Reading it back
 * keeps the attribution dynamic: it is driven by what actually ran, never by
 * a hardcoded provider name (#5136).
 *
 * Only the first line is inspected, and only its *trailing* marker, so neither
 * a `(via …)` string inside a result excerpt nor one inside the echoed query
 * (`Search results for: login (via OAuth) (via Exa)`) can be mistaken for the
 * provider. Returns `undefined` while the call is still running (no result
 * yet) or if no marker is present.
 */
export function extractSearchProvider(result: string | undefined): string | undefined {
  if (!result) return undefined;
  const headingLine = result.split('\n', 1)[0];
  const provider = headingLine?.match(/\(via ([^)]+)\)\s*_?$/i)?.[1]?.trim();
  if (!provider || provider.length > MAX_SEARCH_PROVIDER_LENGTH) return undefined;
  return provider;
}

export interface WebSearchHit {
  title: string;
  url: string;
  domain: string;
  published?: string;
  excerpt?: string;
}

export interface ParsedWebSearch {
  query?: string;
  provider?: string;
  results: WebSearchHit[];
  /** The call completed and found nothing (distinct from "not parseable"). */
  empty: boolean;
}

const MAX_EXCERPT = 280;

function safeHttpUrl(value: string): URL | undefined {
  try {
    const url = new URL(value.trim());
    return url.protocol === 'http:' || url.protocol === 'https:' ? url : undefined;
  } catch {
    return undefined;
  }
}

function clip(text: string | undefined, max = MAX_EXCERPT): string | undefined {
  const cleaned = text?.replace(/\s+/g, ' ').trim();
  if (!cleaned) return undefined;
  return cleaned.length > max ? `${cleaned.slice(0, max - 1)}…` : cleaned;
}

function hit(
  title: string | undefined,
  rawUrl: string | undefined,
  published?: string,
  excerpt?: string
): WebSearchHit | undefined {
  const url = rawUrl ? safeHttpUrl(rawUrl) : undefined;
  if (!url) return undefined;
  const domain = url.hostname.replace(/^www\./, '');
  return {
    title: clip(title, 160) ?? domain,
    url: url.toString(),
    domain,
    ...(clip(published, 40) ? { published: clip(published, 40) } : {}),
    ...(clip(excerpt) ? { excerpt: clip(excerpt) } : {}),
  };
}

function fromStructured(value: unknown): ParsedWebSearch | undefined {
  if (!value || typeof value !== 'object') return undefined;
  const payload = value as Record<string, unknown>;
  if (payload.kind !== 'web_search' || !Array.isArray(payload.results)) return undefined;
  const results = payload.results
    .map(item => {
      if (!item || typeof item !== 'object') return undefined;
      const row = item as Record<string, unknown>;
      const str = (key: string) => (typeof row[key] === 'string' ? (row[key] as string) : undefined);
      return hit(str('title'), str('url'), str('published'), str('excerpt'));
    })
    .filter((row): row is WebSearchHit => row !== undefined);
  return {
    query: typeof payload.query === 'string' ? payload.query : undefined,
    provider: typeof payload.provider === 'string' ? payload.provider : undefined,
    results,
    empty: results.length === 0,
  };
}

/** Strip the trailing `(via X)` marker from a heading's query part. */
function headingQuery(heading: string): string | undefined {
  const query = heading.replace(/\s*\(via [^)]+\)\s*$/i, '').trim();
  return query.replace(/^`|`$/g, '').trim() || undefined;
}

function fromText(text: string): ParsedWebSearch | undefined {
  const lines = text.split('\n');
  const heading = lines[0]?.trim() ?? '';
  const provider = extractSearchProvider(heading);

  const emptyMatch = heading.match(/^_?No (?:\w+ )?results (?:found )?for:?\s*(.+?)_?$/i);
  if (emptyMatch) {
    return {
      query: headingQuery(emptyMatch[1].replace(/_$/, '').replace(/[._]+$/, '')),
      provider,
      results: [],
      empty: true,
    };
  }

  // Markdown rendering.
  const mdHeading = heading.match(/^#\s+\w+ results\s*(?:--|:|—)\s*(.+)$/i);
  if (mdHeading || lines.some(line => /^##\s+\[.+\]\(.+\)\s*$/.test(line))) {
    const results: WebSearchHit[] = [];
    let current: { title: string; url: string; published?: string; excerpt: string[] } | null =
      null;
    const flush = () => {
      if (!current) return;
      const row = hit(current.title, current.url, current.published, current.excerpt.join(' '));
      if (row) results.push(row);
      current = null;
    };
    for (const line of lines.slice(1)) {
      const link = line.match(/^##\s+\[(.+)\]\((\S+)\)\s*$/);
      if (link) {
        flush();
        current = { title: link[1], url: link[2], excerpt: [] };
        continue;
      }
      if (!current) continue;
      const published = line.match(/^_Published:\s*(.+?)_\s*$/);
      if (published) current.published = published[1];
      else if (line.startsWith('>')) current.excerpt.push(line.replace(/^>\s?/, ''));
    }
    flush();
    return {
      query: mdHeading ? headingQuery(mdHeading[1]) : undefined,
      provider,
      results,
      empty: results.length === 0,
    };
  }

  // Plain-text rendering.
  const textHeading = heading.match(/^(?:Search|\w+) results for:\s*(.+)$/i);
  if (!textHeading) return undefined;
  const results: WebSearchHit[] = [];
  let i = 1;
  while (i < lines.length) {
    const item = lines[i].match(/^\s*\d+\.\s+(.+)$/);
    const urlLine = lines[i + 1]?.trim();
    if (!item || !urlLine || !safeHttpUrl(urlLine)) {
      i += 1;
      continue;
    }
    let published: string | undefined;
    const excerpt: string[] = [];
    let j = i + 2;
    for (; j < lines.length; j += 1) {
      const next = lines[j];
      if (/^\s*\d+\.\s+/.test(next) && lines[j + 1] && safeHttpUrl(lines[j + 1].trim())) break;
      const trimmed = next.trim();
      const date = trimmed.match(/^Published:\s*(.+)$/);
      if (date) published = date[1];
      else if (!/^Author:/.test(trimmed) && trimmed) excerpt.push(trimmed);
    }
    const row = hit(item[1], urlLine, published, excerpt.join(' '));
    if (row) results.push(row);
    i = j;
  }
  return {
    query: headingQuery(textHeading[1]),
    provider,
    results,
    empty: results.length === 0,
  };
}

/**
 * Parse a web-search result. `structured` wins when present; otherwise the
 * text `output` is parsed. Returns `undefined` when neither is recognisable,
 * so the caller can fall back to the generic output view.
 */
export function parseWebSearchResult(
  output: unknown,
  structured?: unknown
): ParsedWebSearch | undefined {
  const fromPayload = fromStructured(structured) ?? fromStructured(output);
  if (fromPayload) return fromPayload;
  if (typeof output !== 'string' || !output.trim()) return undefined;
  return fromText(output.trim());
}
