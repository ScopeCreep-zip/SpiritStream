# Bundled font attribution

## SymbolsNerdFont-Brands-Subset.woff2

A **subset** of *Symbols Nerd Font* containing only three brand glyphs used as
service logos in SpiritStream:

| Glyph | Codepoint | Source set |
|-------|-----------|------------|
| Twitch   | U+F1E8 | Font Awesome Free (brands) |
| YouTube  | U+F16A | Font Awesome Free (brands) |
| Facebook | U+F09A | Font Awesome Free (brands) |

- **Symbols Nerd Font** — from [Nerd Fonts](https://github.com/ryanoasis/nerd-fonts),
  MIT License. The font aggregates glyphs from upstream icon sets.
- **The three brand glyphs above** originate from
  [Font Awesome Free](https://fontawesome.com), whose **icons** are licensed
  **CC BY 4.0** (<https://creativecommons.org/licenses/by/4.0/>).

The file was produced with `fonttools` (`pyftsubset --unicodes=F1E8,F16A,F09A
--flavor=woff2`) from the official `NerdFontsSymbolsOnly` release, so only the
glyphs we render are shipped (740 bytes).

Brand logos are trademarks of their respective owners and are used here solely
to identify the corresponding service (nominative use).
