---
description: >-
  Native voice: speech-to-text in, text-to-speech out, mascot lip-sync, and a
  live voice agent you can interrupt mid-sentence.
icon: microphone
---

# Voice

OpenHuman is voice-first when you want it to be. Dictation, reply speech and the live voice agent are part of the core, not a third-party plugin.

## Speech-to-text

* **Hotkey** - push-to-talk and toggle modes.
* **Audio capture** - cross-platform mic capture with voice-activity detection.
* **Streaming transcription** - words appear as you speak.
* **Hallucination filter** - strips well-known artefacts ("Thanks for watching", silence-induced phrases).
* **Postprocess** - punctuation, capitalisation, dictation cleanup.

Dictation can replace the active text input on your desktop, or be sent straight into a chat with the agent.

## Text-to-speech

Reply speech routes through a hosted TTS model. The agent's responses can be spoken back in a voice you pick, with natural timing and prosody. Voice selection is configurable per user, and the mascot avatar lip-syncs to the audio stream via a viseme map.

## Talk to Tiny live

Click Tiny, the mascot in the chat composer, and just talk. Tiny listens and answers out loud in real time, and you can cut in mid-sentence. It uses the same tools as typed chat, under the same tool policy, and anything that needs your approval asks in the conversation. What you both say is saved to the conversation you have open, and Tiny knows what you were just typing about.

Choose how Tiny's voice runs under **Connections → Voice agents**:

| Provider                      | Setup                     | Notes                                                                                                                                        |
| ----------------------------- | ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Gemini Live (TinyHumans)      | None (default)            | One model that hears and speaks. Billed through your TinyHumans balance.                                                                     |
| ElevenLabs Agent (TinyHumans) | None                      | Hosted ElevenLabs voice with OpenHuman as its brain.                                                                                         |
| Gemini Live (Google API key)  | Your Google AI Studio key | Talks to Google directly.                                                                                                                    |
| Sarvam AI                     | Your Sarvam key           | Indian languages (Hindi, Tamil, Bengali and more, or automatic detection). Sarvam's speech recognition, chat model and voices work together. |

Each card has a **Test** button that opens a short session to check the provider. You can also pick the voice, and the language where the provider supports it.

## Privacy

* Audio capture is local. Where the audio goes next depends on the provider you picked: the managed routes send it to the OpenHuman backend, and a bring-your-own-key provider such as Gemini Live on a Google API key talks to that vendor directly. Either way no recording is retained beyond the live transcript.
* TTS audio is streamed and discarded: nothing stored.
* What you and the agent say in a live session is saved to the open conversation, and so reaches your memory engine on the same terms as typed chat, when memory is on.

There is no local speech-to-text engine. The bundled whisper build was removed; STT is either the hosted route or a third-party API you bring a key for. Text-to-speech still has a local option (Piper).

## Removed

The **live Google Meet agent** is gone. It joined a call through the old embedded Chromium webview, transcribed it into memory and spoke back as an outbound camera stream. The frontend surface, the `agent_meetings` domain and the join path were all removed; nothing in the shipped build joins a meeting. The second mascot and its per-mascot voice (the "meeting duo") survive as [mascot](../mascot.md) settings.

## See also

* [The Mascot](../mascot.md): the face that lip-syncs to this audio.
* [Memory](../memory.md): where a spoken conversation ends up.
* [Automatic Model Routing](../model-routing/): live turns want `hint:fast`.
