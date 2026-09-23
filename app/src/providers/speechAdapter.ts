/**
 * Read-aloud for the assistant-ui surface, over OpenHuman's existing TTS.
 *
 * `ActionBarPrimitive.Speak` / `.StopSpeaking` are exported by our pinned
 * assistant-ui and were rendered nowhere, because the capability derives from
 * `adapters.speech` and the external-store adapter supplied none.
 *
 * ## Why this is a bridge and not a new feature
 *
 * The app already speaks. `openhuman.voice_reply_synthesize` dispatches through
 * the user's configured `tts_provider` (cloud / piper / …), and the chat mascot
 * already calls it on this very route — `ChatMascotOverlay` speaks replies when
 * the voice stage is expanded and the "speak replies" switch is on. So this
 * adapter adds no backend, no provider, and no new configuration: it calls the
 * same RPC through the same client.
 *
 * What it adds is the affordance that surface does NOT have. The mascot's
 * speech is automatic, applies to every reply, and only runs while the voice
 * stage is open. A text-chat reader who wants ONE answer read aloud currently
 * has to open the voice stage and enable speak-replies — which then speaks
 * everything from that point and never reads the message they were looking at.
 * This is per-message, on demand, in ordinary text chat.
 *
 * ## Deliberately not the browser's speech synthesis
 *
 * assistant-ui ships `WebSpeechSynthesisAdapter`. Using it would put a second,
 * different-sounding voice in an app that already has one, with none of the
 * provider routing the settings screen offers. It would also repeat the
 * dictation finding (#6489): a Web Speech API that is present but does not
 * behave in this app's webview.
 *
 * Nothing here touches the mascot, Rive, or the lipsync pipeline — those are
 * the visual layer over the same audio, and a text-chat reader wants none of it.
 */
import type { SpeechSynthesisAdapter } from '@assistant-ui/react';

import {
  isAudioStopped,
  type PlaybackHandle,
  playBase64Audio,
} from '../features/human/voice/audioPlayer';
import { prepareForSpeech, synthesizeSpeech } from '../features/human/voice/ttsClient';

/**
 * Safety bound on a single utterance, mirroring the voice path's own guard: a
 * provider that returns a runaway file should not be able to hold the audio
 * element open indefinitely. Ten minutes is far beyond any real answer.
 */
const MAX_UTTERANCE_MS = 10 * 60 * 1000;

type Status = SpeechSynthesisAdapter.Utterance['status'];

/**
 * Speak one message.
 *
 * The returned utterance is live: `status` moves `starting` → `running` →
 * `ended`, and `subscribe` fires on each transition so assistant-ui can swap
 * the Speak button for Stop and back again.
 */
function speak(text: string): SpeechSynthesisAdapter.Utterance {
  const listeners = new Set<() => void>();
  let status: Status = { type: 'starting' };
  let handle: PlaybackHandle | null = null;
  let cancelled = false;

  const setStatus = (next: Status) => {
    if (status.type === 'ended') return;
    status = next;
    for (const listener of listeners) listener();
  };

  void (async () => {
    try {
      // Strip markdown, code fences and link syntax before synthesis — the
      // same preparation the spoken-reply path uses, so a read-aloud sounds
      // like the mascot rather than like someone reading punctuation.
      const spoken = prepareForSpeech(text);
      if (spoken.trim().length === 0) {
        // Nothing sayable (a tool-only or code-only answer). Ending as
        // `finished` rather than `error` keeps the button from flashing a
        // failure at the user for a message that simply has no prose.
        setStatus({ type: 'ended', reason: 'finished' });
        return;
      }

      const reply = await synthesizeSpeech(spoken);
      if (cancelled) return;

      handle = await playBase64Audio(reply.audio_base64, reply.audio_mime, {
        maxDurationMs: MAX_UTTERANCE_MS,
      });
      // `cancel()` can land between the await above and this line, when the
      // handle did not yet exist for it to stop.
      if (cancelled) {
        handle.stop();
        // `stop()` rejects `ended` by design. Every other path reaches the
        // `await` below and lets the `catch` absorb it; this one returns
        // first, so the rejection would have no handler attached and surface
        // as an unhandled promise rejection.
        void handle.ended.catch(() => {});
        return;
      }

      setStatus({ type: 'running' });
      await handle.ended;
      setStatus({ type: 'ended', reason: 'finished' });
    } catch (error) {
      // `stop()` rejects `ended` by design, so a cancellation arrives here as
      // an error and must not be reported as one.
      if (cancelled || isAudioStopped(error)) {
        setStatus({ type: 'ended', reason: 'cancelled' });
        return;
      }
      setStatus({ type: 'ended', reason: 'error', error });
    }
  })();

  return {
    get status() {
      return status;
    },
    cancel() {
      cancelled = true;
      handle?.stop();
      setStatus({ type: 'ended', reason: 'cancelled' });
    },
    subscribe(callback: () => void) {
      listeners.add(callback);
      return () => {
        listeners.delete(callback);
      };
    },
  };
}

/** The `adapters.speech` entry for {@link useOpenHumanExternalStore}. */
export const openHumanSpeechAdapter: SpeechSynthesisAdapter = { speak };
