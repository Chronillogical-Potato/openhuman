---
description: >-
  Five theme families with light, dark and auto variants, a visual Theme Studio
  for colours, fonts and backdrops, and Appearance controls for text size,
  corner rounding and borders.
icon: palette
---

# Themes & Theme Studio

OpenHuman is re-skinnable at runtime. Every change applies instantly and persists locally, with no restart.

The controls sit in two places:

| Page | What you set there |
| --- | --- |
| **Settings → Appearance** (`/settings/appearance`) | Text size, corner rounding, border contrast, per-area borders |
| **Settings → Theme Studio** (`/settings/theme`) | Theme family, light/dark/auto, every colour token, fonts per role, the backdrop, and custom-theme management |

---

## Built-in themes

Five families ship in `app/src/lib/theme/presets.ts`, each with a light and a dark variant:

| Family | Feel | Default variant |
| --- | --- | --- |
| **Classic** | The default OpenHuman look. | Light |
| **Ocean** | Cool blues. | Light |
| **Sepia** | Warm and paper-like, set in a serif. | Light |
| **Matrix** | High-contrast green on black, set in a monospace. | Dark |
| **HAL 9000** | Deep black with a red accent. | Dark |

Each family is applied as **Light**, **Dark** or **Auto**. Auto follows your operating system's `prefers-color-scheme` and re-applies live the moment you flip the OS between light and dark, with no reload.

Classic carries no colour or font overrides of its own on purpose: it is the palette defined in `app/src/styles/tokens.css`, selected by nothing more than the light/dark switch.

---

## Colours

The Theme Studio exposes every colour token, grouped as **Surfaces**, **Text**, **Borders** and **Accent colours**. The four accent ramps (primary, sage, amber, coral) show their `500` shade by default and expand to all eleven shades from `50` to `950`.

Each token is stored as a space-separated RGB channel triple such as `255 255 255`, not a hex string, because Tailwind wraps every token as `rgb(var(--token) / <alpha-value>)` so opacity modifiers like `bg-surface/50` keep working. The reasoning is recorded at the top of `app/src/styles/tokens.css`.

A **contrast warning** appears when the luminance gap between `content` and `surface-canvas` falls under 0.2. It is advisory and does not block the change.

---

## Fonts

Five roles can each take a different family:

| Role | Used for |
| --- | --- |
| `title` | Display and brand type |
| `heading` | Section headings |
| `body` | Everything else |
| `mono` | Code and fixed-width output |
| `serif` | Serif passages |

All five pick from the same six choices in `app/src/lib/theme/tokens.ts` (`FONT_CHOICES`): Inter, Cabinet Grotesk, System UI, Newsreader (serif), Georgia (serif) and JetBrains Mono. A role whose stored stack matches no choice shows a disabled **Current** entry rather than silently snapping to another font.

Cabinet Grotesk is not bundled, so every stack naming it resolves to Inter. The name is kept so the intended stack stays legible; see the note in `app/src/styles/tokens.css`.

---

## Backdrop

Three kinds, set in the Theme Studio's **Background** card:

| Kind | What it paints |
| --- | --- |
| `solid` | Nothing of its own: the themed body colour shows through. The default. |
| `mesh` | An animated, theme-tinted WebGL mesh gradient. |
| `image` | A cover image from a URL you supply. |

No built-in preset sets a backdrop, so every family starts on `solid` and the animated shader is opt-in. The dotted-canvas overlay that once sat above all three was removed outright rather than defaulted off, so there is no setting to rediscover.

---

## Text size

Four tiers, applied as an inline `font-size` on the root `<html>` element so everything rem-based scales with it:

| Tier | Size |
| --- | --- |
| Small | 14px |
| Medium | 16px (default) |
| Large | 18px |
| Extra large | 20px |

Beyond the tiers, a **Custom size** number field and slider take any whole pixel value. It is clamped to **12px to 28px** by `clampFontSizePx` in `app/src/store/themeSlice.ts`, which brackets the tiers on both sides: denser than Small, larger than Extra large. A non-finite value falls back to 16. Picking a tile clears the custom value.

---

## Layout

Three controls in `app/src/lib/theme/layout.ts`, all under Appearance, with a **Reset layout** button that restores every default.

**Corner rounding** scales Tailwind's whole radius scale (`--radius-xs` through `--radius-5xl`) by one factor:

| Option | Factor |
| --- | --- |
| None | 0 |
| Subtle | 0.5 |
| Default | 1 (the variables are removed, so the stylesheet wins) |
| Round | 1.6 |

Pills and avatars use `rounded-full` rather than a radius token, so they stay round at any setting.

**Border contrast** adjusts `--line`, `--line-strong` and `--line-subtle`, leaving `--line-chrome` to the theme. The tokens are reset before each change, so levels never compound:

| Option | Effect |
| --- | --- |
| Subtle | Mixes the line tokens 55 percent toward the surface |
| Default | No change: the theme's own values stand |
| Strong | Mixes the line tokens 30 percent toward the text colour |

**Per-area borders** turn borders off for one area at a time, by setting a `data-borders-<area>="off"` attribute that `app/src/styles/layout-overrides.css` acts on. Four areas:

| Area | Covers |
| --- | --- |
| Cards & panels | Settings cards, grouped sections and panels |
| Form controls | Text fields, text boxes and dropdowns |
| Row dividers | Hairlines between rows in lists |
| Window frame | The edge around the main content area |

---

## Custom themes

Changing any colour, font or backdrop on a built-in preset forks it into a new custom theme named `<Name> (custom)` and makes that active. The preset stays pristine, so you can start from Ocean, tweak it, and keep both. Forking happens once per source theme.

A custom theme also offers **Reset overrides** (back to the preset it was forked from, or empty if it was not) and **Delete theme**, which falls back to Classic.

Sharing goes through JSON on the clipboard, not a file. **Copy JSON** puts the whole theme object in your clipboard and also renders it in a read-only text box to copy by hand. Import is a paste field: the JSON needs a `colors` object whose values are all strings, an empty one is accepted, and an unrecognised backdrop kind is dropped rather than failing the import. An imported theme gets a fresh id, so importing never overwrites what you have.

---

## How a theme is applied

A theme is just a set of values for the tokens in `app/src/styles/tokens.css`. `applyTheme` in `app/src/providers/ThemeProvider.tsx` writes the active theme's colours and fonts as inline `--token` custom properties on the root `<html>` element, overriding the stylesheet's light and dark blocks at runtime.

**Presets and fully custom themes are the same mechanism.** There is no per-theme stylesheet class and no `data-theme` attribute. The only class involved is `.dark`, a light/dark base selector that also keeps residual Tailwind `dark:` utilities aligned. Any token a theme leaves out falls through to the stylesheet defaults, and tokens set by a previous theme are cleared on each switch so nothing leaks.

Theme state persists through `redux-persist` under the `theme` key in plain `localStorage` rather than `userScopedStorage`, deliberately: the theme is a pre-login, whole-app preference, so it survives switching users instead of being scoped to one.

---

## Not wired

Two fields in `app/src/store/themeSlice.ts` are persisted but inert. Neither has a UI control, and neither has a consumer:

| Field | State |
| --- | --- |
| `developerMode` | Has a reducer and a `selectDeveloperMode` selector, but nothing dispatches or reads either. Its own doc comment points at a `useDeveloperMode` hook that does not exist. Developer Options gates on core mode instead. |
| `tabBarLabels` | Has a reducer and no selector at all, so there is not even a readable accessor. |

Both sit in the persist whitelist, so a stale default may sit in `localStorage` doing nothing. Editing either by hand has no visible effect.

---

## See also

- [Theming (contributor reference)](../developing/theming.md): the token taxonomy, Tailwind wiring and component-authoring rules.
- [The Mascot](mascot/README.md): the other large piece of OpenHuman's personality.
- [Platform & Availability](platform.md): which desktop platforms the shell ships on.
