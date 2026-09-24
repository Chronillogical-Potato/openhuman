/**
 * Dev-only gallery of tool-call presentation (`/dev/tools`).
 *
 * Renders the chat's tool-call card and assistant-ui tool timeline with
 * realistic payloads in every state, plus the whole core tool catalog with
 * each tool's icon and both tenses, so a label or icon regression is visible
 * at a glance. Registered only in dev builds (see `AppRoutes.tsx`).
 */
import { useState } from 'react';

import { ToolTimeline } from '../../components/assistant-ui/elements/tool-timeline';
import { AssistantUiToolCallCard } from '../../features/conversations/components/AssistantUiToolCall';
import coreToolNames from '../../features/conversations/tools/__fixtures__/coreToolNames.json';
import { ToolIcon } from '../../features/conversations/tools/ToolIcon';
import { describeToolCall, toolLabel } from '../../features/conversations/tools/toolPresentation';
import { useT } from '../../lib/i18n/I18nContext';

const SEARCH_RESULT = [
  'Search results for: rust async traits (via Exa)',
  '1. Announcing async fn and return-position impl Trait in traits',
  '   https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits.html',
  '   Published: 2023-12-21',
  '   The Rust Async Working Group is excited to announce major progress.',
  '2. async-trait crate',
  '   https://docs.rs/async-trait/latest/async_trait/',
  '   Type erasure for async trait methods.',
  '3. Async in traits: the design',
  '   https://smallcultfollowing.com/babysteps/blog/2019/10/26/async-fn-in-traits-are-hard/',
  '4. Tokio tutorial',
  '   https://tokio.rs/tokio/tutorial',
].join('\n');

const SAMPLES = [
  {
    toolName: 'web_search_tool',
    args: { query: 'rust async traits' },
    result: SEARCH_RESULT,
    status: 'success' as const,
    elapsedMs: 1840,
  },
  {
    toolName: 'web_search_tool',
    args: { query: 'tauri v2 deep links' },
    status: 'running' as const,
  },
  {
    toolName: 'file_read',
    args: { path: 'crates/openhuman-core/src/agent/progress.rs' },
    result: 'pub enum AgentProgress {\n    ToolCallStarted { .. },\n}',
    status: 'success' as const,
    elapsedMs: 12,
  },
  {
    toolName: 'edit',
    args: {
      path: 'app/src/App.tsx',
      old_string: 'const theme = "light";',
      new_string: 'const theme = useTheme();\nconst accent = theme.accent;',
    },
    result: 'ok',
    status: 'success' as const,
    elapsedMs: 40,
  },
  {
    toolName: 'shell',
    args: { command: 'pnpm test --run tools' },
    result:
      ' ✓ toolPresentation.test.ts (22)\n ✓ parseWebSearchResult.test.ts (8)\n\n Test Files  2 passed',
    status: 'success' as const,
    elapsedMs: 5230,
  },
  {
    toolName: 'web_fetch',
    args: { url: 'https://docs.rs/tokio/latest/tokio/' },
    result:
      'status=200 url=https://docs.rs/tokio/latest/tokio/ content=markdown\n# Tokio\n\nA runtime for writing **reliable** asynchronous applications with Rust.',
    status: 'success' as const,
    elapsedMs: 620,
  },
  {
    toolName: 'GMAIL_SEND_EMAIL',
    args: { to: 'alex@example.com', subject: 'Q3 plan' },
    result: '{"successful":true}',
    status: 'success' as const,
    elapsedMs: 910,
  },
  {
    toolName: 'mcp_call_tool',
    args: { server: 'linear', tool: 'create_issue', arguments: { title: 'Fix labels' } },
    status: 'running' as const,
  },
  {
    toolName: 'memory',
    args: { action: 'recall', query: 'preferred meeting times' },
    result: 'Mornings before 11am.',
    status: 'success' as const,
    elapsedMs: 88,
  },
  {
    toolName: 'grep',
    args: { pattern: 'display_label' },
    status: 'error' as const,
    result: 'regex parse error',
    failure: {
      class: 'InvalidInput',
      category: 'Recoverable',
      recoverable: true,
      causePlain: 'The search pattern was not a valid regular expression.',
      nextAction: 'The agent will retry with an escaped pattern.',
    },
  },
  { toolName: 'cron', args: { action: 'add', name: 'Daily digest' }, status: 'cancelled' as const },
  { toolName: 'some_new_tool', args: { name: 'widget' }, status: 'success' as const, result: 'ok' },
];

function CatalogRow({ name }: { name: string }) {
  const { t } = useT();
  const running = describeToolCall({ name, status: 'running' });
  const done = describeToolCall({ name, status: 'success' });
  return (
    <li className="flex items-center gap-2 py-1 text-xs" data-testid="tool-gallery-catalog-row">
      <ToolIcon presentation={done} className="text-foreground/50 size-3.5" />
      <span className="text-foreground/40 w-56 shrink-0 truncate font-mono">{name}</span>
      <span className="text-foreground/80 w-56 shrink-0 truncate">{toolLabel(running, t)}</span>
      <span className="text-foreground/60 truncate">{toolLabel(done, t)}</span>
    </li>
  );
}

export default function ToolCallGallery() {
  const { t } = useT();
  const [streaming, setStreaming] = useState(true);
  return (
    <div className="bg-background text-foreground min-h-screen overflow-auto p-8">
      <div className="mx-auto flex max-w-3xl flex-col gap-10">
        <header>
          <h1 className="text-lg font-semibold">Tool calls</h1>
          <p className="text-foreground/50 text-sm">
            assistant-ui tool-call, tool-timeline and web-search elements over the presentation
            registry.
          </p>
        </header>

        <section className="flex flex-col gap-3">
          <label className="text-foreground/60 flex items-center gap-2 text-xs">
            <input
              type="checkbox"
              checked={streaming}
              onChange={event => setStreaming(event.target.checked)}
            />
            Timeline streaming
          </label>
          <ToolTimeline
            className="max-w-none"
            defaultOpen
            streaming={streaming}
            activeLabel={toolLabel(
              describeToolCall({ name: 'web_search_tool', status: 'running' }),
              t
            )}
            restingLabel="4 steps · Searched the web, Read file, Edited file, Ran command">
            {SAMPLES.slice(0, 5).map((sample, index) => (
              <AssistantUiToolCallCard key={index} {...sample} />
            ))}
          </ToolTimeline>
        </section>

        <section className="flex flex-col gap-1">
          <h2 className="text-foreground/60 mb-2 text-xs font-medium uppercase">Every state</h2>
          {SAMPLES.map((sample, index) => (
            <AssistantUiToolCallCard key={index} {...sample} />
          ))}
          <AssistantUiToolCallCard
            toolName="composio_execute"
            args={{ tool: 'SLACK_SEND_MESSAGE' }}
            awaitingUser
            footer={<p className="text-foreground/50 ps-5 text-xs">(approval card renders here)</p>}
          />
        </section>

        <section>
          <h2 className="text-foreground/60 mb-2 text-xs font-medium uppercase">
            Core catalog ({(coreToolNames as string[]).length})
          </h2>
          <ul className="divide-foreground/[0.06] divide-y">
            {(coreToolNames as string[]).map(name => (
              <CatalogRow key={name} name={name} />
            ))}
          </ul>
        </section>
      </div>
    </div>
  );
}
