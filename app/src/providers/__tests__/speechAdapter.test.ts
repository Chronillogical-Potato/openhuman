/**
 * The read-aloud bridge.
 *
 * assistant-ui drives this through a small contract — `speak(text)` returns an
 * utterance whose `status` moves `starting` → `running` → `ended` and whose
 * `subscribe` fires on each transition. Getting the TERMINAL states right is
 * what matters: the Stop button renders while `status` is not `ended`, so an
 * utterance that never reaches `ended` leaves a Stop button on a message that
 * is silent, and one that reports `error` for an ordinary cancel flashes a
 * failure at a user who simply pressed Stop.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AudioStoppedError, playBase64Audio } from '../../features/human/voice/audioPlayer';
import { synthesizeSpeech } from '../../features/human/voice/ttsClient';
import { openHumanSpeechAdapter } from '../speechAdapter';

vi.mock('../../features/human/voice/ttsClient', async importOriginal => {
  const actual = await importOriginal<typeof import('../../features/human/voice/ttsClient')>();
  return { ...actual, synthesizeSpeech: vi.fn() };
});
vi.mock('../../features/human/voice/audioPlayer', async importOriginal => {
  const actual = await importOriginal<typeof import('../../features/human/voice/audioPlayer')>();
  return { ...actual, playBase64Audio: vi.fn() };
});

/** A playback handle whose `ended` this test resolves or rejects by hand. */
function fakeHandle() {
  let resolveEnded!: () => void;
  let rejectEnded!: (err: Error) => void;
  const ended = new Promise<void>((res, rej) => {
    resolveEnded = res;
    rejectEnded = rej;
  });
  const stop = vi.fn(() => rejectEnded(new AudioStoppedError()));
  return {
    handle: {
      currentMs: () => 0,
      durationMs: () => 0,
      metadataReady: Promise.resolve(),
      stop,
      ended,
    },
    finish: resolveEnded,
    stop,
  };
}

/** Let the adapter's internal promise chain advance. */
const tick = () => new Promise(resolve => setTimeout(resolve, 0));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(synthesizeSpeech).mockResolvedValue({
    audio_base64: 'AAAA',
    audio_mime: 'audio/mpeg',
    visemes: [],
  } as never);
});

describe('openHumanSpeechAdapter', () => {
  it('synthesises the prose and plays it, ending as finished', async () => {
    const { handle, finish } = fakeHandle();
    vi.mocked(playBase64Audio).mockResolvedValue(handle as never);

    const utterance = openHumanSpeechAdapter.speak('Hello **there**.');
    expect(utterance.status.type).toBe('starting');

    await tick();
    expect(utterance.status.type).toBe('running');
    // Markdown is stripped before synthesis, so the reading does not say
    // "asterisk asterisk there asterisk asterisk".
    expect(vi.mocked(synthesizeSpeech).mock.calls[0]?.[0]).not.toContain('**');
    expect(vi.mocked(synthesizeSpeech).mock.calls[0]?.[0]).toContain('there');

    finish();
    await tick();
    expect(utterance.status).toEqual({ type: 'ended', reason: 'finished' });
  });

  it('notifies subscribers on every transition', async () => {
    const { handle, finish } = fakeHandle();
    vi.mocked(playBase64Audio).mockResolvedValue(handle as never);
    const utterance = openHumanSpeechAdapter.speak('Hello.');
    const onChange = vi.fn();
    utterance.subscribe(onChange);

    await tick();
    finish();
    await tick();

    // Without this the Stop button never appears and never goes away: the
    // button is rendered from state assistant-ui only re-reads when told to.
    expect(onChange.mock.calls.length).toBeGreaterThanOrEqual(2);
  });

  it('ends as cancelled — not error — when the reader presses Stop', async () => {
    // `PlaybackHandle.stop()` rejects `ended` by design, so a cancel arrives on
    // the error path and must not be reported as a failure.
    const { handle, stop } = fakeHandle();
    vi.mocked(playBase64Audio).mockResolvedValue(handle as never);
    const utterance = openHumanSpeechAdapter.speak('Hello.');
    await tick();

    utterance.cancel();
    await tick();

    expect(stop).toHaveBeenCalled();
    expect(utterance.status).toEqual({ type: 'ended', reason: 'cancelled' });
  });

  it('stops audio that only started after the cancel landed', async () => {
    // The race: cancel() between `synthesizeSpeech` resolving and the handle
    // existing. Without the post-await check the reader presses Stop and the
    // audio starts playing anyway, with no handle left to stop it.
    const { handle, stop } = fakeHandle();
    let releasePlayback!: (h: unknown) => void;
    vi.mocked(playBase64Audio).mockReturnValue(
      new Promise(resolve => {
        releasePlayback = resolve;
      }) as never
    );

    const utterance = openHumanSpeechAdapter.speak('Hello.');
    await tick();
    utterance.cancel();
    releasePlayback(handle);
    await tick();

    expect(stop).toHaveBeenCalled();
    expect(utterance.status).toEqual({ type: 'ended', reason: 'cancelled' });
  });

  it('reports a synthesis failure as an error', async () => {
    vi.mocked(synthesizeSpeech).mockRejectedValue(new Error('tts provider unreachable'));

    const utterance = openHumanSpeechAdapter.speak('Hello.');
    await tick();

    expect(utterance.status.type).toBe('ended');
    expect(utterance.status).toMatchObject({ reason: 'error' });
  });

  it('finishes quietly for a message with no prose to read', async () => {
    // A tool-only or code-only answer. Ending as `finished` rather than
    // `error` keeps the button from flashing a failure at a message that
    // simply has nothing sayable in it.
    const utterance = openHumanSpeechAdapter.speak('```ts\nconst x = 1;\n```');
    await tick();

    expect(utterance.status).toEqual({ type: 'ended', reason: 'finished' });
    expect(vi.mocked(synthesizeSpeech)).not.toHaveBeenCalled();
  });
});
