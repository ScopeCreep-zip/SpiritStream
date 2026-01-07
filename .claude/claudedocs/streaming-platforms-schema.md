# Streaming Platforms JSON Schema

## Enhanced Service Object

```typescript
interface StreamingService {
  // Required fields (from OBS)
  name: string;              // Full name: "YouTube / YouTube Gaming"
  defaultUrl: string;        // RTMP URL: "rtmp://a.rtmp.youtube.com/live2"
  streamKeyPlacement: string; // "append" (most common)

  // Optional UI fields
  displayName?: string;      // Short name for UI: "YouTube" (if name is too long)
  abbreviation?: string;     // 1-2 chars: "YT", "TW", "K"
  color?: string;           // Brand color: "#FF0000"
  faviconPath?: string;     // Path to icon: "icons/platforms/youtube.svg"
}
```

## Example Entries

### Major Platform (All Fields)
```json
{
  "name": "YouTube / YouTube Gaming",
  "displayName": "YouTube",
  "defaultUrl": "rtmp://a.rtmp.youtube.com/live2",
  "streamKeyPlacement": "append",
  "abbreviation": "YT",
  "color": "#FF0000",
  "faviconPath": "icons/platforms/youtube.svg"
}
```

### Simple Platform (Minimal Fields)
```json
{
  "name": "AngelThump",
  "defaultUrl": "rtmp://ingest.angelthump.com/live",
  "streamKeyPlacement": "append"
}
```

### Platform with Long Name (displayName)
```json
{
  "name": "Bilibili Live - RTMP | 哔哩哔哩直播 - RTMP",
  "displayName": "Bilibili",
  "defaultUrl": "rtmp://live-push.bilivideo.com/live-bvc/",
  "streamKeyPlacement": "append",
  "abbreviation": "BL",
  "color": "#00A1D6"
}
```

## Field Guidelines

### `name` (required)
- Full official name from OBS
- Used for exact matching and search
- Can be long: "YouTube / YouTube Gaming"

### `displayName` (optional)
- Shortened name for UI dropdowns and cards
- Use when `name` is too long (>20 chars recommended)
- Example: "YouTube / YouTube Gaming" → "YouTube"
- If omitted, `name` is used

### `abbreviation` (optional)
- 1-2 uppercase characters for icon badges
- Shown when no favicon is available
- Examples: "YT", "TW", "K", "FB", "BL"
- Auto-generated if omitted (first 1-2 letters)

### `color` (optional)
- Hex color code with #
- Official brand color from platform guidelines
- Used for icon backgrounds
- If omitted, auto-generated from name hash

### `faviconPath` (optional)
- Relative path from project root
- Recommended: `icons/platforms/{slug}.svg`
- SVG preferred, PNG acceptable
- Size: 32x32px or larger (square)
- If omitted, uses colored abbreviation badge

### `defaultUrl` (required)
- Complete RTMP(S) URL without stream key
- Used for platform detection and normalization

### `streamKeyPlacement` (required)
- "append" - Stream key appended to URL (most common)
- "query" - Stream key in query parameter (rare)

## Icon/Favicon Organization

```
public/
  icons/
    platforms/
      youtube.svg       # Major platforms with custom icons
      twitch.svg
      kick.svg
      facebook.svg
      trovo.svg
      bilibili.svg
      # ... others as added
```

## Usage Examples

### UI Display
```tsx
// Use displayName if available, otherwise name
const displayText = service.displayName || service.name;

// Show icon
if (service.faviconPath) {
  return <img src={service.faviconPath} alt={displayText} />;
} else {
  // Fallback to colored badge
  const abbr = service.abbreviation || generateAbbreviation(service.name);
  const color = service.color || generateColor(service.name);
  return <ColoredBadge text={abbr} color={color} />;
}
```

### Search
```tsx
// Search both name and displayName
const searchResults = services.filter(s =>
  s.name.toLowerCase().includes(query) ||
  s.displayName?.toLowerCase().includes(query)
);
```

## Migration Strategy

### Phase 1: Add Optional Fields Manually
- Add fields to top 15-20 major platforms
- Test UI rendering
- Verify favicon loading

### Phase 2: Community Contributions
- Accept PRs for additional platforms
- Guidelines for brand colors and favicons
- Auto-validate with schema

### Phase 3: Auto-Generation Fallbacks
- Generate abbreviation from name if missing
- Hash-based color if color not provided
- Ensure all platforms look good

## Validation Schema

```typescript
const ServiceSchema = {
  required: ['name', 'defaultUrl', 'streamKeyPlacement'],
  properties: {
    name: { type: 'string', minLength: 1 },
    displayName: { type: 'string', minLength: 1 },
    defaultUrl: { type: 'string', pattern: '^rtmps?://' },
    streamKeyPlacement: { enum: ['append', 'query'] },
    abbreviation: {
      type: 'string',
      minLength: 1,
      maxLength: 2,
      pattern: '^[A-Z0-9]+$'
    },
    color: {
      type: 'string',
      pattern: '^#[0-9A-Fa-f]{6}$'
    },
    faviconPath: {
      type: 'string',
      pattern: '^icons/platforms/[a-z0-9-]+\\.(svg|png)$'
    }
  }
};
```

## Recommended Curated List (Start Here)

Update these platforms first with full metadata:

1. YouTube / YouTube Gaming
2. Twitch
3. Kick
4. Facebook Live
5. TikTok Live
6. Trovo
7. Rumble
8. DLive
9. Nimo TV
10. Bilibili Live
11. Huya
12. Douyu
13. Mildom
14. Streamlabs
15. Restream.io

Others can use auto-generation fallbacks until manually curated.

---

**Created**: 2026-01-06
**Status**: Schema design ready for implementation
