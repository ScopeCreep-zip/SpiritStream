# SpiritStream Themes

This directory contains built-in example themes for SpiritStream. Each theme is provided as a single mode file (`light` or `dark`). If you want both modes, create two files (naming is up to you; `-light`/`-dark` is just a convention).

## Available Themes

- Spirit Light / Spirit Dark
- Dracula Light / Dracula Dark
- Nord Light / Nord Dark
- Catppuccin Mocha Light / Catppuccin Mocha Dark
- Rainbow Pride Light / Rainbow Pride Dark
- Trans Pride Light / Trans Pride Dark

## How to Install Themes

### Method 1: Settings UI (Recommended)

1. Open SpiritStream.
2. Navigate to **Settings > Themes**.
3. Click **Install Theme**.
4. Select a theme file (for example: `dracula-dark.jsonc` or `nord-light.jsonc`).

The theme will be validated and copied to your AppData directory.

### Method 2: Manual Installation

1. Locate your SpiritStream AppData directory:
   - **Windows**: `%APPDATA%\com.spiritstream.desktop\themes\`
   - **macOS**: `~/Library/Application Support/com.spiritstream.desktop/themes/`
   - **Linux**: `~/.local/share/com.spiritstream.desktop/themes/`

2. Copy the desired `.jsonc` theme file to this directory.
3. Restart SpiritStream or wait for the theme list to refresh.

## How to Use Themes

1. Go to **Settings > Themes**.
2. Select a theme from the dropdown.
3. The theme applies immediately.

## Creating Your Own Theme

Use `theme-template.jsonc` as a starting point:

```bash
# Copy the template
cp theme-template.jsonc my-theme-light.jsonc
```

Then edit the file:

1. Set a unique `id` (lowercase, dashes/underscores only).
2. Set a friendly `name`.
3. Set `mode` to `light` or `dark`.
4. Replace all `REPLACE_ME` values with valid CSS values.

If you want both modes, create a second file (for example, `my-theme-dark.jsonc`) with `mode: "dark"`.

### Theme Structure

```jsonc
{
  "id": "my-theme-light",
  "name": "My Theme Light",
  "mode": "light",
  "tokens": {
    "--primary": "#7C3AED",
    "--bg-base": "#FAF8F5"
    // ...all required tokens
  }
}
```

## Required Tokens

The required token list is derived from `src-frontend/styles/tokens.css`. Each theme file must provide all required tokens in its `tokens` map.

## Troubleshooting

### Theme does not appear in the list

- Ensure the file is valid JSON or JSONC.
- Make sure the file has a `.json` or `.jsonc` extension.
- Verify the `id` matches: `^[a-z0-9][a-z0-9-_]{0,63}$`.
- Check logs for validation errors.

### Theme looks broken

- Verify all CSS values are valid (hex, rgb/rgba, hsl/hsla, gradients, etc.).
- Ensure sizes include units (px, rem, %, etc.).
- Compare against the built-in examples in this folder.

## License

- Example themes (Dracula, Nord, Catppuccin, Pride variants): MIT License.
- Theme template: MIT License.
- SpiritStream application: GPL-3.0.
