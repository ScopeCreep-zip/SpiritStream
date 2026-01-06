# Themes

This document explains SpiritStream themes and how to create custom ones.

## Overview

- Themes are JSON or JSONC files (JSON with comments).
- Each file represents a single mode: `light` or `dark`.
- If you want both variants, create two separate themes (naming is up to you; `-light`/`-dark` is just a convention).
- Built-in themes include **Spirit Light** and **Spirit Dark**.
- Custom themes are loaded from the app data themes folder and reloaded dynamically.

## Theme File Format

```jsonc
{
  "id": "sunset-light",
  "name": "Sunset Light",
  "mode": "light",
  "tokens": {
    "--bg-base": "#FAF8F5",
    "--primary": "#FF6A3D"
    // ...all required tokens
  }
}
```

Notes:
- `id` must be lowercase alphanumeric with `-` or `_` only.
- `mode` must be `light` or `dark`.
- JSONC supports `//` and `/* */` comments.
- Trailing commas are not supported.

## Required Tokens

The required token list is derived from `src-frontend/styles/tokens.css`.
Each theme file must include every required token in its `tokens` map.

## Where Themes Live

Custom themes are stored in the app data directory:

- Windows: `%APPDATA%\com.spiritstream.desktop\themes`
- macOS: `~/Library/Application Support/com.spiritstream.desktop/themes`
- Linux: `~/.local/share/com.spiritstream.desktop/themes`

The app watches this folder and reloads themes automatically.

## Installing a Theme

1. Create a `.json` or `.jsonc` file.
2. Open **Settings > Themes**.
3. Click **Install Theme** and select the file.

If validation fails, the theme is rejected and not copied.

## Reference and Template

- `themes/spirit-light.jsonc` and `themes/spirit-dark.jsonc` are reference exports of the built-in Spirit theme.
- `themes/theme-template.jsonc` is a single-mode template with placeholders.
  - Replace every `REPLACE_ME` value before installing.
