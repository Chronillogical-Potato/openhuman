/**
 * The composer's `/` command source: OpenHuman glue that feeds assistant-ui's
 * `unstable_useSlashCommandAdapter` for the vendored `ComposerTriggerPopover`.
 *
 * Three sources, merged by id (first wins):
 * 1. Local builtins — `/new`, `/clear`, `/stop`, `/plan`, `/build`. Each routes
 *    through {@link handleComposerSlashCommand}, the same decision the typed
 *    path in `Conversations` uses: `new_or_clear` runs the registry's
 *    `chat.new` action, `run_mode` calls {@link useRunMode}'s `setMode`, and
 *    `stop` cancels through the runtime's `cancelRun` (→ the chat surface's
 *    registered cancel, i.e. `handleStopGeneration`).
 * 2. The core's catalog, `openhuman.commands_list` (`{ id, label,
 *    description?, kind: builtin|skill|workflow, insert? }`). That RPC is being
 *    added by a parallel core workstream; until it exists the call rejects and
 *    this falls back to no core commands.
 * 3. Registry actions that declare a `slashCommand` ({@link useSlashCommands}).
 */
import {
  type Unstable_IconComponent,
  type Unstable_SlashCommand,
  unstable_useSlashCommandAdapter,
  useAui,
} from '@assistant-ui/react';
import debug from 'debug';
import {
  EraserIcon,
  HammerIcon,
  ListChecksIcon,
  PlusIcon,
  SlashIcon,
  SparklesIcon,
  SquareIcon,
  WorkflowIcon,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';

import { registry } from '../../../lib/commands/registry';
import { useSlashCommands } from '../../../lib/commands/useSlashCommands';
import { useT } from '../../../lib/i18n/I18nContext';
import { callCoreRpc } from '../../../services/coreRpcClient';
import { handleComposerSlashCommand } from '../composerSendDecision';
import { useRunMode } from './useRunMode';

const log = debug('openhuman:chat:slash-commands');

export type CoreCommandKind = 'builtin' | 'skill' | 'workflow';

/** One entry of `openhuman.commands_list`. */
export interface CoreCommand {
  id: string;
  label: string;
  description?: string;
  kind: CoreCommandKind;
  /** Composer text to insert when chosen; defaults to `/<id> `. */
  insert?: string;
}

const BUILTIN_IDS = ['new', 'clear', 'stop', 'plan', 'build'] as const;

const BUILTIN_DESCRIPTIONS: Record<(typeof BUILTIN_IDS)[number], [key: string, en: string]> = {
  new: ['conversations.composer.command.new', 'Start a new conversation'],
  clear: ['conversations.composer.command.clear', 'Clear the conversation'],
  stop: ['conversations.composer.command.stop', 'Stop the running reply'],
  plan: [
    'conversations.composer.command.plan',
    'Plan first: review the steps before anything runs',
  ],
  build: ['conversations.composer.command.build', 'Build: let the agent act directly'],
};

const ICON_MAP: Record<string, Unstable_IconComponent> = {
  new: PlusIcon,
  clear: EraserIcon,
  stop: SquareIcon,
  plan: ListChecksIcon,
  build: HammerIcon,
  skill: SparklesIcon,
  workflow: WorkflowIcon,
};

const KINDS: readonly string[] = ['builtin', 'skill', 'workflow'];

function isCoreCommand(value: unknown): value is CoreCommand {
  if (!value || typeof value !== 'object') return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v.id === 'string' &&
    v.id.length > 0 &&
    typeof v.label === 'string' &&
    typeof v.kind === 'string' &&
    KINDS.includes(v.kind) &&
    (v.description === undefined || typeof v.description === 'string') &&
    (v.insert === undefined || typeof v.insert === 'string')
  );
}

/**
 * `openhuman.commands_list`, unwrapped and validated. Never rejects: a core
 * without the method (or any transport failure) yields `[]`.
 */
export async function fetchCoreCommands(): Promise<CoreCommand[]> {
  try {
    const resp = await callCoreRpc<unknown>({ method: 'openhuman.commands_list' });
    const inner =
      resp && typeof resp === 'object' && !Array.isArray(resp) && 'data' in resp
        ? (resp as { data: unknown }).data
        : resp;
    const list = Array.isArray(inner)
      ? inner
      : ((inner as { commands?: unknown } | null)?.commands ?? []);
    const commands = Array.isArray(list) ? list.filter(isCoreCommand) : [];
    log('commands_list: %d command(s)', commands.length);
    return commands;
  } catch (error) {
    log('commands_list unavailable, using local commands only: %o', error);
    return [];
  }
}

/** Builtins, then core commands, then registry commands — first id wins. */
export function mergeSlashCommands({
  builtins,
  core,
  registry: registryCommands,
  insert,
}: {
  builtins: readonly Unstable_SlashCommand[];
  core: readonly CoreCommand[];
  registry: readonly Unstable_SlashCommand[];
  insert: (text: string) => void;
}): Unstable_SlashCommand[] {
  const seen = new Set<string>();
  const out: Unstable_SlashCommand[] = [];
  const add = (command: Unstable_SlashCommand) => {
    if (seen.has(command.id)) return;
    seen.add(command.id);
    out.push(command);
  };
  builtins.forEach(add);
  for (const entry of core) {
    add({
      id: entry.id,
      description: entry.description ?? entry.label,
      icon: entry.kind,
      execute: () => insert(entry.insert ?? `/${entry.id} `),
    });
  }
  registryCommands.forEach(add);
  return out;
}

/** Spreadable props for `<ComposerTriggerPopover char="/" … />`. */
export function useSlashCommandSource(threadId: string | null) {
  const { t } = useT();
  const aui = useAui();
  const { setMode } = useRunMode(threadId);
  const registryCommands = useSlashCommands();
  const [core, setCore] = useState<CoreCommand[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    void fetchCoreCommands().then(commands => {
      if (cancelled) return;
      setCore(commands);
      setIsLoading(false);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const commands = useMemo(() => {
    const runBuiltin = (id: string) => {
      const decision = handleComposerSlashCommand(`/${id}`);
      log('builtin /%s -> %s', id, decision.kind);
      switch (decision.kind) {
        case 'new_or_clear':
          registry.runAction('chat.new');
          return;
        case 'run_mode':
          void setMode(decision.mode).catch(error => log('set run mode failed: %o', error));
          return;
        case 'stop':
          try {
            aui.thread().cancelRun();
          } catch (error) {
            log('cancel failed: %o', error);
          }
          return;
        case 'not_handled':
          return;
      }
    };
    const builtins: Unstable_SlashCommand[] = BUILTIN_IDS.map(id => {
      const [key, fallback] = BUILTIN_DESCRIPTIONS[id];
      return { id, description: t(key, fallback), icon: id, execute: () => runBuiltin(id) };
    });
    const insert = (text: string) => {
      const current = aui.composer().getState().text;
      aui.composer().setText(`${text}${current}`);
    };
    return mergeSlashCommands({ builtins, core, registry: registryCommands, insert });
  }, [aui, core, registryCommands, setMode, t]);

  const slash = unstable_useSlashCommandAdapter({
    commands,
    removeOnExecute: true,
    iconMap: ICON_MAP,
    fallbackIcon: SlashIcon,
  });

  return {
    ...slash,
    isLoading,
    backLabel: t('conversations.composer.trigger.back', 'Back'),
    loadingLabel: t('conversations.composer.trigger.loading', 'Loading…'),
    emptyCategoriesLabel: t('conversations.composer.trigger.emptyCategories', 'No items available'),
    emptyItemsLabel: t('conversations.composer.slash.empty', 'No matching commands'),
  };
}
