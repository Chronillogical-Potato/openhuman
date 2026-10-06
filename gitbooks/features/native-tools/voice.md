---
description: >-
  Native voice - speech-to-text in, text-to-speech out, mascot lip-sync,
  and a live Google Meet agent that listens and speaks.
icon: microphone
---

# Voice

OpenHuman is voice-first when you want it to be. STT, TTS, and the live Google Meet agent are part of the core, not a third-party plugin.

## Speech-to-text

- **Hotkey** - push-to-talk and toggle modes.
- **Audio capture** - cross-platform mic capture with voice-activity detection.
- **Streaming transcription** - words appear as you speak.
- **Hallucination filter** - strips well-known artefacts ("Thanks for watching", silence-induced phrases).
- **Postprocess** - punctuation, capitalisation, dictation cleanup.

Dictation can replace the active text input on your desktop, or be sent straight into a chat with the agent.

## Text-to-speech

Reply speech routes through a hosted TTS model. The agent's responses can be spoken back in a voice you pick, with natural timing and prosody. Voice selection is configurable per user, and the mascot avatar lip-syncs to the audio stream via a viseme map.

## Talk to Tiny live

Click Tiny, the mascot in the chat composer, and just talk. Tiny listens and answers out loud in real time, and you can cut in mid-sentence. It uses the same tools as typed chat, under the same tool policy, and anything that needs your approval asks in the conversation. What you both say is saved to the conversation you have open, and Tiny knows what you were just typing about.

Choose how Tiny's voice runs under **Connections → Voice agents**:

| Provider | Setup | Notes |
| --- | --- | --- |
| Gemini Live (TinyHumans) | None (default) | One model that hears and speaks. Billed through your TinyHumans balance. |
| ElevenLabs Agent (TinyHumans) | None | Hosted ElevenLabs voice with OpenHuman as its brain. |
| Gemini Live (Google API key) | Your Google AI Studio key | Talks to Google directly. |
| Sarvam AI | Your Sarvam key | Indian languages (Hindi, Tamil, Bengali and more, or automatic detection). Sarvam's speech recognition, chat model and voices work together. |

Each card has a **Test** button that opens a short session to check the provider. You can also pick the voice, and the language where the provider supports it.

## Live Google Meet agent

OpenHuman's flagship voice integration:

- Joins a Google Meet via the embedded webview.
- Streams audio out to STT in real time, transcribes everyone in the call, and writes structured notes into the [Memory](../memory.md) as the meeting progresses.
- When you ask it to speak (or it decides it has something useful to add), it generates audio through the TTS model and **plays it back into the meeting as an outbound camera/mic stream**, so other participants actually hear it.

## Privacy

- Audio capture is local. Streaming STT goes through the OpenHuman backend; no recording is retained beyond the live transcript.
- TTS audio is streamed and discarded - nothing stored.
- Meeting transcripts are stored as conversations in your memory engine, when memory is on.

## See also

- [Memory](../memory.md) - where Meet transcripts and notes live.
- [Automatic Model Routing](../model-routing/) - Meet's brain uses `hint:fast` for low-latency conversational turns.
