# SpiritStream Themes

This directory contains example themes for SpiritStream. These themes demonstrate the theme system and can be used as templates for creating your own custom themes.

## Available Themes

| Theme | Description | License |
|-------|-------------|---------|
| **Dracula** | Popular dark theme with purple, pink, and cyan accents | MIT |
| **Nord** | Arctic-inspired color palette with frost blue tones | MIT |
| **Catppuccin Mocha** | Soothing pastel theme with excellent contrast | MIT |

## How to Install Themes

### Method 1: Via Settings UI (Recommended)

1. Open SpiritStream
2. Navigate to **Settings** (⚙️ icon in sidebar)
3. Scroll to the **Themes** section
4. Click **"Install Theme"** or **"Browse for Theme"**
5. Navigate to the `themes/` folder in your SpiritStream installation
6. Select a theme file (e.g., `dracula.jsonc`, `nord.jsonc`, `catppuccin-mocha.jsonc`)
7. Click **Open** to install

The theme will be validated and copied to your AppData directory.

### Method 2: Manual Installation

1. Locate your SpiritStream AppData directory:
   - **Windows**: `%APPDATA%\com.spiritstream.app\themes\`
   - **macOS**: `~/Library/Application Support/com.spiritstream.app/themes/`
   - **Linux**: `~/.local/share/com.spiritstream.app/themes/`

2. Copy the desired `.jsonc` theme file to this directory

3. Restart SpiritStream or the theme should hot-reload automatically

## How to Use Themes

After installing a theme:

1. Go to **Settings** > **Appearance**
2. Under **Theme**, select your installed theme from the dropdown
3. The theme will apply immediately
4. Toggle between **Light** and **Dark** mode to see both variants

## Creating Your Own Theme

Use `theme-template.jsonc` as a starting point:

```bash
# Copy the template
cp theme-template.jsonc my-custom-theme.jsonc

# Edit the theme
# 1. Change "id" to a unique lowercase identifier (e.g., "my-theme")
# 2. Change "name" to your theme's display name
# 3. Replace all REPLACE_ME values with valid CSS colors
```

### Theme Structure

```jsonc
{
  "id": "unique-theme-id",        // Lowercase, alphanumeric, dashes, underscores
  "name": "My Custom Theme",      // Display name
  "tokens": {
    "light": {
      "--primary": "#7C3AED",     // Your light mode colors
      // ... 163 tokens total
    },
    "dark": {
      "--primary": "#A78BFA",     // Your dark mode colors
      // ... 163 tokens total
    }
  }
}
```

## CSS Token Reference

### Required Tokens

All themes must provide these token categories in both `light` and `dark` modes:

| Category | Examples | Count |
|----------|----------|-------|
| **Primary Colors** | `--primary`, `--primary-hover`, `--primary-subtle` | ~6 |
| **Secondary Colors** | `--secondary`, `--secondary-hover` | ~6 |
| **Accent Colors** | `--accent`, `--accent-hover` | ~7 |
| **Backgrounds** | `--bg-base`, `--bg-surface`, `--bg-elevated` | ~14 |
| **Text Colors** | `--text-primary`, `--text-secondary`, `--text-tertiary` | ~6 |
| **Borders** | `--border-default`, `--border-interactive` | ~7 |
| **Status Colors** | `--success`, `--warning`, `--error`, `--info` | ~16 |
| **Stream Status** | `--status-live`, `--status-connecting`, `--status-error` | ~8 |
| **Shadows** | `--shadow-sm`, `--shadow-md`, `--shadow-lg` | ~7 |
| **Gradients** | `--gradient-brand`, `--gradient-surface` | ~3 |
| **Focus Ring** | `--ring-default`, `--ring-offset` | ~4 |
| **Color Scales** | `--pink-50` through `--pink-900`, etc. | ~90 |

**Total**: 163 tokens per mode (326 total)

### Validation

Themes are validated on installation:

✅ **Valid CSS values:**
- Hex colors: `#7C3AED`, `#RRGGBB`, `#RRGGBBAA`
- RGB/RGBA: `rgb(124, 58, 237)`, `rgba(124, 58, 237, 0.9)`
- HSL/HSLA: `hsl(258, 80%, 58%)`
- CSS variables: `var(--primary-hover)`
- Named colors: `rebeccapurple`, `transparent`
- Sizes with units: `1.5rem`, `260px`, `100%`
- Shadows: `0 4px 6px rgba(0, 0, 0, 0.1)`
- Gradients: `linear-gradient(135deg, #7C3AED 0%, #C026D3 100%)`

❌ **Invalid values will be rejected:**
- Invalid hex: `#ZZZ`, `#12345`
- Missing units: `260` (should be `260px`)
- Malicious content: `</style>`, `<script>`
- Empty values: `""`

## Troubleshooting

### Theme doesn't appear in list

- Check that the file is valid JSON/JSONC
- Ensure the `id` field matches the pattern: `^[a-z0-9][a-z0-9-_]{0,63}$`
- Make sure all required tokens are present
- Check the console/logs for validation errors

### Theme looks broken

- Verify all CSS color values are valid
- Check that sizes include units (px, rem, %, etc.)
- Ensure gradients are properly formatted
- Compare against the example themes

### Can't install theme

- Make sure the theme file has `.jsonc` or `.json` extension
- Check file permissions on the themes directory
- Try copying the file manually to the AppData folder
- Check application logs for detailed error messages

## Color Palette Resources

When creating your own theme, these resources can help:

- **Color Palette Generators**:
  - [Coolors.co](https://coolors.co/)
  - [Adobe Color](https://color.adobe.com/)
  - [Paletton](https://paletton.com/)

- **Existing Theme Ports**:
  - [Dracula](https://draculatheme.com/)
  - [Nord](https://www.nordtheme.com/)
  - [Catppuccin](https://github.com/catppuccin/catppuccin)
  - [Tokyo Night](https://github.com/enkia/tokyo-night-vscode-theme)
  - [Gruvbox](https://github.com/morhetz/gruvbox)

- **Color Contrast Checkers** (for accessibility):
  - [WebAIM Contrast Checker](https://webaim.org/resources/contrastchecker/)
  - [Colorable](https://colorable.jxnblk.com/)

## Theme Guidelines

### Accessibility

- Maintain WCAG 2.2 AA contrast ratios:
  - Text on background: At least 4.5:1
  - Large text (18pt+): At least 3:1
  - UI components: At least 3:1

### Consistency

- Use your primary color for interactive elements (buttons, links)
- Use semantic colors appropriately (green for success, red for error)
- Maintain visual hierarchy with text color weights
- Keep shadow depths consistent

### Testing

Test your theme with:
- Both light and dark modes
- All stream statuses (live, offline, connecting, error)
- Different UI states (hover, active, disabled)
- Long text content and edge cases

## Contributing

Found an issue with an example theme or want to add a new one?

1. Fork the repository
2. Create your theme in the `themes/` directory
3. Test it thoroughly in both light and dark modes
4. Submit a pull request with:
   - Theme file (`.jsonc`)
   - Screenshots of both modes
   - Description and color palette info

## License

- **Example themes** (Dracula, Nord, Catppuccin): MIT License (see individual theme files)
- **Theme template**: MIT License
- **SpiritStream application**: GPL-3.0

---

**Need Help?**

- 📖 [Full Documentation](../.claude/claudedocs/)
- 🐛 [Report Issues](https://github.com/ScopeCreep-zip/SpiritStream/issues)
- 💬 [Discussions](https://github.com/ScopeCreep-zip/SpiritStream/discussions)

**Last Updated**: 2026-01-05
