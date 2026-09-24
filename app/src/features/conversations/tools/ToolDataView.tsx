import { BubbleMarkdown } from '../components/AgentMessageBubble';

/**
 * Generic, readable rendering of a tool's input or output: JSON objects as a
 * definition list, arrays as a list, strings as markdown. The fallback body
 * for any tool without a dedicated renderer.
 */

function friendlyLabel(key: string): string {
  return key
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .replace(/[_-]+/g, ' ')
    .replace(/^./, char => char.toUpperCase());
}

export function parsedValue(value: unknown): unknown {
  if (typeof value !== 'string') return value;
  const trimmed = value.trim();
  if (!(trimmed.startsWith('{') || trimmed.startsWith('['))) return value;
  try {
    return JSON.parse(trimmed);
  } catch {
    return value;
  }
}

export function hasDisplayValue(value: unknown): boolean {
  if (value === undefined || value === null || value === '') return false;
  if (Array.isArray(value)) return value.length > 0;
  if (typeof value === 'object') return Object.keys(value as object).length > 0;
  return true;
}

export function ToolDataView({ value }: { value: unknown }) {
  const parsed = parsedValue(value);
  if (Array.isArray(parsed)) {
    return (
      <ul className="space-y-1 text-xs">
        {parsed.map((item, index) => (
          <li key={index} className="bg-muted/50 rounded-md px-2 py-1.5">
            <ToolDataView value={item} />
          </li>
        ))}
      </ul>
    );
  }
  if (parsed && typeof parsed === 'object') {
    const entries = Object.entries(parsed);
    for (const key of ['content', 'output', 'result', 'message', 'query', 'q']) {
      const semantic = entries.find(([candidate]) => candidate === key)?.[1];
      if (hasDisplayValue(semantic)) return <ToolDataView value={semantic} />;
    }
    return (
      <dl className="divide-border bg-muted/40 divide-y rounded-md px-2 text-xs">
        {entries.map(([key, item]) => (
          <div key={key} className="grid grid-cols-[minmax(7rem,auto)_1fr] gap-3 py-1.5">
            <dt className="text-muted-foreground font-medium">{friendlyLabel(key)}</dt>
            <dd className="min-w-0 wrap-break-word">
              <ToolDataView value={item} />
            </dd>
          </div>
        ))}
      </dl>
    );
  }
  if (typeof parsed === 'boolean') return <span>{parsed ? 'Yes' : 'No'}</span>;
  if (typeof parsed === 'string') return <BubbleMarkdown content={parsed} />;
  return <span className="whitespace-pre-wrap">{String(parsed ?? '')}</span>;
}
