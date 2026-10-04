# Japanese UI validation

Validation performed on 2026-10-04 after the locale refresh at
`673e33a87f0936582173304818d4b84bef80ea1b`.
The follow-up changes add tests and documentation; the application code in the
native build is that exact revision.

## Automated browser E2E

`app/test/playwright/specs/japanese-locale.spec.ts` imports the shared suite in
`app/test/e2e/specs/japanese-locale.browser.ts`. The existing Playwright web lane
discovers the wrapper. Run the focused suite with the repository's mock backend
and standalone Core:

```sh
pnpm --filter openhuman-app test:e2e:web:build
bash app/scripts/e2e-web-session.sh test/playwright/specs/japanese-locale.spec.ts
```

All three tests passed:

- Select 日本語 through Settings, verify Japanese Chat labels and `html[lang=ja]`,
  reload and verify persistence, then select English and reload again.
- Start a fresh `ja-JP` browser context, verify automatic Japanese selection,
  and verify that a saved English choice wins after reload.
- At 1280×720, navigate to Memory v2 Explorer and Token Usage. Verify `7件`,
  `Qwen3.8-Flash-Next`, and `3 回の圧縮で` replace the interpolation tokens and
  remain visible. Only data-dependent read RPC results are fixtures; locale
  changes, storage, navigation and authentication use the shared Core harness.

E2E TypeScript, ESLint, Prettier and the coverage-matrix guard passed. Using Node
24, the focused LanguageSelect, localeSlice, i18n and attachment suites passed
150 tests. A prior run with mixed Node versions failed one gzip attachment test;
that failure disappeared when the runtime matched CI.

## Native macOS smoke

Built the production frontend and unsigned debug `.app` with the PR's pinned
Rust toolchain and recursive submodules. Used a separate application identifier
and disposable workspace, leaving the installed app and production memory
intact. No WebDriver or mocked model was used for these native checks.

Observed:

- Fresh first-run UI displayed 日本語 in the language/runtime picker.
- Settings, Chat, Connections, Memory v2 and Gateway labels rendered in Japanese.
- Settings switched Japanese → English → Japanese immediately.
- Actual app quit/relaunch checks preserved English and Japanese.
- The remote Core connection test succeeded over an SSH loopback tunnel.
- The Japanese routing dialog tested a custom OpenAI-compatible provider through
  the real model router and received `Hello! How can I help you today?` from
  Qwen3.8-Flash-Next. The provider and model name were visible in the response.
- After assigning that custom provider to Chat, a new native conversation sent
  a Japanese request for `17×19` and streamed the correct answer `323`.
  The UI → remote Core → actual router → model → UI path completed.
  After another quit/relaunch, both Japanese labels and that conversation
  remained visible.

## Integration limits

The existing production Core rejected the new UI's `reasoning_effort` parameter
on `channel.web_chat`. Therefore the native smoke used a separately built Core
at the same revision, running on the remote Mac Studio with isolated data.
Production Core and memory were not upgraded or migrated.

The legacy `local-openai` agent path reached the actual model but did not complete
a simple arithmetic request: its main wire request contained a synthetic
`Continue with the task described above.` user turn and the model attempted
unrelated tools. That legacy-path run was unsuccessful. The same real
endpoint configured through the current custom-provider UI did complete the
native chat E2E above; production routing was not changed. The isolated Memory
service was not configured, so Memory Explorer
showed its connection error; successful item/count rendering is covered by the
browser fixture test above. No iOS or Android native smoke was performed.
