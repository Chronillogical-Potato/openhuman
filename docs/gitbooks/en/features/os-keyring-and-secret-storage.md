---
description: >-
  How OpenHuman uses the OS keyring to protect local secrets, what stays
  encrypted on disk, and what the consent prompt asks.
icon: key
---

# OS keyring and secret storage

OpenHuman uses your operating system's secure credential store to protect the secrets that must live on your device:

- **macOS:** Keychain
- **Windows:** Credential Manager
- **Linux:** Secret Service (libsecret)

This is the root of trust for local secrets. OpenHuman does not keep user credentials in a plaintext `.env` file or a plaintext config file.

## What goes into the keyring

The keyring holds two kinds of secret.

**Credential entries.** When a feature needs a local credential slot, OpenHuman stores it in the platform keyring instead of a normal config file. Examples are locally stored provider API keys, session and bearer tokens that must stay on the device, and wallet secret material where applicable. Entries sit under OpenHuman's own key namespace, so they do not collide with other apps.

**The master encryption key.** Some sensitive values must live inside local files, because the app's configuration is file-based. OpenHuman splits the storage. The secret value is stored on disk as encrypted ciphertext. The master key that decrypts it lives in the OS keyring. Your config and state files can hold encrypted values without the decryption key sitting next to them.

## What stays encrypted on disk

When OpenHuman saves a sensitive setting locally, it writes the ciphertext to disk and keeps the key in the keyring. That covers:

- BYO API keys for supported providers.
- Channel and webhook secrets stored in local config.
- Other locally saved secret settings that desktop features need.

The encryption is authenticated, so OpenHuman detects tampering instead of silently accepting changed ciphertext. In short: the key is in the keyring, the ciphertext is in the file, and plaintext exists only in memory when needed.

## Why this beats plaintext config

Plaintext secrets in config files are a risk if you have a workspace backup, a sync folder or a support bundle. With the keyring as the root secret store:

- You can copy config files without exposing raw credentials.
- Accidental log or file inspection is less likely to reveal secrets.
- The decryption key belongs to the platform's credential system, not to a plaintext file the app manages.

This does not replace full-disk encryption or OS account security. It is a narrower, stronger way to handle application secrets.

## Managed integrations and local secrets

In the default managed integration flow, the OpenHuman backend handles third-party OAuth tokens. Your app does not need to keep those provider tokens in plaintext on your machine.

When you choose a bring-your-own-key or direct-mode path, OpenHuman treats those credentials as local secrets. It protects them with the OS keyring and encrypted local storage where needed.

## Migrating from older installs

Older versions could keep local encryption material in a file. Current desktop builds move that material into the OS keyring and keep the encrypted values on disk. You do not need to re-enter your secrets.

## When the keyring is unavailable

Sometimes the keyring cannot be reached. On Linux that can mean no Secret Service daemon. On macOS it can mean keychain access was denied. When that happens, OpenHuman stops and asks before it falls back to local encrypted storage.

1. **Detection.** On startup the core probes the OS keychain. If the probe fails, it classifies the reason (no daemon, locked, denied) and reports a structured `KeyringStatus` through the `openhuman.keyring_consent_status` RPC and the app snapshot.
2. **Consent prompt.** The first time a secret must be read or written without recorded consent, a modal explains what happened, what "store locally" means and the risks. You can choose:
   - **Use Local Encrypted Storage:** consent to ChaCha20-Poly1305 encrypted files. The master key is also on disk.
   - **Retry OS Keychain:** probe again, which helps after you grant OS permission.
   - **Skip:** decline local storage. Features that need secrets will be unavailable.
3. **Saved choice.** Your choice is recorded in `app-state.json` (the `keyringConsent` field) and cached in the process. The app probes again on each launch and asks again if the keyring becomes available after a local-only session.
4. **Settings.** **Settings → Security** shows the active storage mode, keychain availability and failure reason, with buttons to retry or change consent.

### One fallback policy

Auth profiles, config secrets, the wallet mnemonic and the `secrets.enc` backend all call `keyring_consent::policy::check_secret_access()` instead of checking `is_available()` directly. No code path switches storage modes silently.

| Policy decision | Meaning |
| --- | --- |
| `Proceed` | The OS keyring is available, or you consented to local encrypted storage. |
| `ConsentRequired` | The keyring is unavailable and there is no consent yet. Block and prompt. |
| `Declined` | You refused local storage. Skip the secret operation. |

## Platform note

This page describes the desktop app (Tauri) on macOS, Windows and Linux. Development and test environments may use test-specific overrides so automated runs do not need an interactive keychain. That is a developer convenience, not the end-user security model.

## See also

- [Privacy and security](privacy-and-security.md)
- [Third-party integrations](integrations/README.md)
- [Local AI (optional)](model-routing/local-ai.md)
