---
version: alpha
name: DropGrab
description: >
  UI design system and screen specification for DropGrab, an app that sends files to any
  nearby device and downloads files, videos, and music from links. One accent color,
  neutral surfaces, a 4/8 spacing scale, and text contrast of at least 4.5:1.
colors:
  primary: "#12161A"
  secondary: "#444C56"
  muted: "#58616C"
  neutral: "#FFFFFF"
  surface: "#F4F5F7"
  sunken: "#E8EBEE"
  outline: "#5C6570"
  divider: "#E1E4E8"
  accent: "#1646D6"
  accent-pressed: "#0F36AE"
  accent-tint: "#E6EDFE"
  on-accent: "#FFFFFF"
  dark-primary: "#F2F4F6"
  dark-secondary: "#C3C9D0"
  dark-muted: "#A3ABB5"
  dark-neutral: "#0E1114"
  dark-surface: "#171B20"
  dark-sunken: "#232A31"
  dark-outline: "#8A939E"
  dark-divider: "#2A3138"
  dark-accent: "#8FB0FF"
  dark-accent-pressed: "#B4CBFF"
  dark-accent-tint: "#16254F"
  dark-on-accent: "#0B1020"
typography:
  display:
    fontFamily: Inter
    fontSize: 32px
    fontWeight: 700
    lineHeight: 40px
    letterSpacing: -0.01em
  headline:
    fontFamily: Inter
    fontSize: 24px
    fontWeight: 700
    lineHeight: 32px
  title:
    fontFamily: Inter
    fontSize: 20px
    fontWeight: 600
    lineHeight: 28px
  body:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: 400
    lineHeight: 24px
  body-sm:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: 400
    lineHeight: 20px
  label:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: 600
    lineHeight: 20px
  caption:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: 500
    lineHeight: 16px
  numeric:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: 500
    lineHeight: 24px
    fontFeature: "tnum"
rounded:
  sm: 4px
  md: 8px
  lg: 16px
  full: 9999px
spacing:
  xs: 4px
  sm: 8px
  md: 16px
  lg: 24px
  xl: 32px
  xxl: 48px
  xxxl: 64px
components:
  screen:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.primary}"
    typography: "{typography.body}"
    padding: "{spacing.md}"
  screen-dark:
    backgroundColor: "{colors.dark-neutral}"
    textColor: "{colors.dark-primary}"
    typography: "{typography.body}"
    padding: "{spacing.md}"
  headline:
    textColor: "{colors.primary}"
    typography: "{typography.headline}"
  headline-dark:
    textColor: "{colors.dark-primary}"
    typography: "{typography.headline}"
  support-text:
    textColor: "{colors.secondary}"
    typography: "{typography.body}"
  support-text-dark:
    textColor: "{colors.dark-secondary}"
    typography: "{typography.body}"
  meta-text:
    textColor: "{colors.muted}"
    typography: "{typography.body-sm}"
  meta-text-dark:
    textColor: "{colors.dark-muted}"
    typography: "{typography.body-sm}"
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.on-accent}"
    typography: "{typography.label}"
    rounded: "{rounded.md}"
    height: 48px
    padding: "{spacing.md}"
  button-primary-pressed:
    backgroundColor: "{colors.accent-pressed}"
    textColor: "{colors.on-accent}"
  button-primary-dark:
    backgroundColor: "{colors.dark-accent}"
    textColor: "{colors.dark-on-accent}"
    typography: "{typography.label}"
    rounded: "{rounded.md}"
    height: 48px
    padding: "{spacing.md}"
  button-primary-dark-pressed:
    backgroundColor: "{colors.dark-accent-pressed}"
    textColor: "{colors.dark-on-accent}"
  button-secondary:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.accent}"
    typography: "{typography.label}"
    rounded: "{rounded.md}"
    height: 48px
    padding: "{spacing.md}"
  button-secondary-dark:
    backgroundColor: "{colors.dark-neutral}"
    textColor: "{colors.dark-accent}"
    typography: "{typography.label}"
    rounded: "{rounded.md}"
    height: 48px
    padding: "{spacing.md}"
  input:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.primary}"
    typography: "{typography.body}"
    rounded: "{rounded.md}"
    height: 48px
    padding: "{spacing.md}"
  input-dark:
    backgroundColor: "{colors.dark-neutral}"
    textColor: "{colors.dark-primary}"
    typography: "{typography.body}"
    rounded: "{rounded.md}"
    height: 48px
    padding: "{spacing.md}"
  control-outline:
    backgroundColor: "{colors.outline}"
    height: 1px
  control-outline-dark:
    backgroundColor: "{colors.dark-outline}"
    height: 1px
  divider:
    backgroundColor: "{colors.divider}"
    height: 1px
  divider-dark:
    backgroundColor: "{colors.dark-divider}"
    height: 1px
  device-tile:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.primary}"
    typography: "{typography.title}"
    rounded: "{rounded.lg}"
    height: 80px
    padding: "{spacing.md}"
  device-tile-selected:
    backgroundColor: "{colors.accent-tint}"
    textColor: "{colors.primary}"
  device-tile-dark:
    backgroundColor: "{colors.dark-surface}"
    textColor: "{colors.dark-primary}"
    typography: "{typography.title}"
    rounded: "{rounded.lg}"
    height: 80px
    padding: "{spacing.md}"
  device-tile-dark-selected:
    backgroundColor: "{colors.dark-accent-tint}"
    textColor: "{colors.dark-primary}"
  card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.primary}"
    typography: "{typography.body}"
    rounded: "{rounded.lg}"
    padding: "{spacing.md}"
  card-dark:
    backgroundColor: "{colors.dark-surface}"
    textColor: "{colors.dark-primary}"
    typography: "{typography.body}"
    rounded: "{rounded.lg}"
    padding: "{spacing.md}"
  list-item:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.primary}"
    typography: "{typography.body}"
    height: 64px
    padding: "{spacing.md}"
  list-item-dark:
    backgroundColor: "{colors.dark-neutral}"
    textColor: "{colors.dark-primary}"
    typography: "{typography.body}"
    height: 64px
    padding: "{spacing.md}"
  chip:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.primary}"
    typography: "{typography.label}"
    rounded: "{rounded.full}"
    height: 40px
    padding: "{spacing.md}"
  chip-selected:
    backgroundColor: "{colors.accent-tint}"
    textColor: "{colors.accent}"
  chip-dark:
    backgroundColor: "{colors.dark-surface}"
    textColor: "{colors.dark-primary}"
    typography: "{typography.label}"
    rounded: "{rounded.full}"
    height: 40px
    padding: "{spacing.md}"
  chip-dark-selected:
    backgroundColor: "{colors.dark-accent-tint}"
    textColor: "{colors.dark-accent}"
  tab-item:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.secondary}"
    typography: "{typography.caption}"
    height: 64px
  tab-item-active:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.accent}"
  tab-item-dark:
    backgroundColor: "{colors.dark-neutral}"
    textColor: "{colors.dark-secondary}"
    typography: "{typography.caption}"
    height: 64px
  tab-item-dark-active:
    backgroundColor: "{colors.dark-neutral}"
    textColor: "{colors.dark-accent}"
  progress-track:
    backgroundColor: "{colors.sunken}"
    rounded: "{rounded.full}"
    height: 8px
  progress-fill:
    backgroundColor: "{colors.accent}"
    rounded: "{rounded.full}"
    height: 8px
  progress-track-dark:
    backgroundColor: "{colors.dark-sunken}"
    rounded: "{rounded.full}"
    height: 8px
  progress-fill-dark:
    backgroundColor: "{colors.dark-accent}"
    rounded: "{rounded.full}"
    height: 8px
  banner:
    backgroundColor: "{colors.sunken}"
    textColor: "{colors.primary}"
    typography: "{typography.body-sm}"
    rounded: "{rounded.md}"
    padding: "{spacing.md}"
  banner-dark:
    backgroundColor: "{colors.dark-sunken}"
    textColor: "{colors.dark-primary}"
    typography: "{typography.body-sm}"
    rounded: "{rounded.md}"
    padding: "{spacing.md}"
  sheet:
    backgroundColor: "{colors.neutral}"
    textColor: "{colors.primary}"
    typography: "{typography.body}"
    rounded: "{rounded.lg}"
    padding: "{spacing.lg}"
  sheet-dark:
    backgroundColor: "{colors.dark-neutral}"
    textColor: "{colors.dark-primary}"
    typography: "{typography.body}"
    rounded: "{rounded.lg}"
    padding: "{spacing.lg}"
---

# DropGrab UI Design

This document follows the DESIGN.md format: the YAML block above holds the exact design tokens, and the sections below explain why each value exists and how to apply it. Where the two disagree, the tokens win.

## Overview

DropGrab does one thing first: **it sends files to any nearby device.** Everything else (downloading links, the browser, the library) supports that moment or follows from it.

The interface feels like a well-set page of plain text with one confident color. It is quiet, left-aligned, and fast to scan. There are no gradients, no illustrations, no decorative motion, and no second color competing with the first.

### The seven rules

These rules govern every screen. Section "Rule compliance" at the end shows where each one is enforced.

| # | Rule | What it means here |
|---|---|---|
| 1 | One accent color plus neutrals | A single blue family (`accent`) is the only chromatic color. Everything else is gray |
| 2 | The headline says what it does | Every screen opens with a headline that names the action or the contents, never a greeting or a slogan |
| 3 | One hero feature | Sending to a nearby device owns the home screen and the first-run screen. Nothing else gets hero treatment |
| 4 | Left-aligned text, one icon set | All text is left-aligned. All icons come from one set (Lucide) at one stroke weight |
| 5 | A 4/8 spacing scale | Every gap, padding, size, and line height is a multiple of 4, and layout gaps are multiples of 8 |
| 6 | Contrast of at least 4.5:1 | All text and icons reach 4.5:1 against their background in both light and dark themes |
| 7 | Real content, no placeholders | Every mockup uses believable device names, file names, sizes, and times. No lorem ipsum, "Item 1", or "User Name" |

### Brand

- Name: **DropGrab**, set as a text wordmark in Inter 700 at the `headline` size. No logo mark in v1.
- Voice: plain, specific, and brief. Verbs first. No exclamation marks.
- Themes: light and dark, following the system setting, with a manual override in Settings.

### Hero feature: Send to a nearby device

The hero is the list of nearby devices on the home screen. It is the largest element on the screen, sits above everything else, and reaches a finished transfer in three taps: **Choose files, tap a device, wait for the receiver to accept.**

Supporting features (Downloads, Browser, Library) are one tap away in the tab bar, and the only download entry on the home screen is a single quiet link field below the hero.

## Colors

The palette is one accent hue and a set of neutrals. Light and dark themes each have their own token values. Dark values carry a `dark-` prefix.

### Light theme

| Token | Value | Role |
|---|---|---|
| `primary` | `#12161A` | Ink. Headlines and body text |
| `secondary` | `#444C56` | Supporting text and inactive navigation |
| `muted` | `#58616C` | Metadata such as sizes, times, and sources |
| `neutral` | `#FFFFFF` | Screen background |
| `surface` | `#F4F5F7` | Cards, device tiles, chips |
| `sunken` | `#E8EBEE` | Progress tracks and banners |
| `outline` | `#5C6570` | Borders of inputs and controls, and icon strokes that sit alone |
| `divider` | `#E1E4E8` | Decorative hairlines between list rows (never the only cue that something is a control) |
| `accent` | `#1646D6` | The one color: primary buttons, links, active tab, progress, focus ring, selected state |
| `accent-pressed` | `#0F36AE` | Pressed state of `accent` |
| `accent-tint` | `#E6EDFE` | Selected rows and chips, a pale tone of the same hue |
| `on-accent` | `#FFFFFF` | Text and icons on `accent` |

### Dark theme

| Token | Value | Role |
|---|---|---|
| `dark-primary` | `#F2F4F6` | Headlines and body text |
| `dark-secondary` | `#C3C9D0` | Supporting text and inactive navigation |
| `dark-muted` | `#A3ABB5` | Metadata |
| `dark-neutral` | `#0E1114` | Screen background |
| `dark-surface` | `#171B20` | Cards, device tiles, chips |
| `dark-sunken` | `#232A31` | Progress tracks and banners |
| `dark-outline` | `#8A939E` | Borders of inputs and controls |
| `dark-divider` | `#2A3138` | Decorative hairlines |
| `dark-accent` | `#8FB0FF` | The one color, in its lighter tone for dark backgrounds |
| `dark-accent-pressed` | `#B4CBFF` | Pressed state |
| `dark-accent-tint` | `#16254F` | Selected rows and chips |
| `dark-on-accent` | `#0B1020` | Text and icons on `dark-accent` |

### One accent, two tones

The accent is one hue. A single value cannot reach 4.5:1 on both white and near-black, so dark mode uses the same hue at a lighter tone. Both tones are the same color to the user, and no other hue appears anywhere in the product.

### How the accent is used

Accent marks **what you can act on or what is happening now**:

- The primary button on each screen (one per screen).
- Links and secondary-button text.
- The active tab icon and label.
- Progress fills and the transfer-speed readout while active.
- The focus ring.
- Selected state (a tinted background plus a check icon).

Accent is never used for decoration, headlines, illustrations, or backgrounds of large areas.

### Status without extra colors

Because the palette has no red, green, or yellow, status is carried by **icon + words + position**:

| State | Icon (Lucide) | Wording pattern | Presentation |
|---|---|---|---|
| Working | `loader` (rotating) or progress bar | "Sending 3 files" | Accent progress fill |
| Done | `check` | "Sent 3 files in 21 s" | `primary` text, check icon |
| Paused | `pause` | "Paused at 41%" | `secondary` text |
| Needs attention | `triangle-alert` | States the problem and the fix | `banner` with `primary` text |
| Failed | `circle-x` | States what failed and what to do next | `banner` with `primary` text |
| Destructive action | `trash-2` | Names the exact thing deleted | Confirmation dialog with the item name in the title |

If the product later needs a semantic red, add it as one documented exception in this file with its contrast values. It is not part of v1.

### Contrast

All text and icons meet 4.5:1 in both themes. The measured ratios are listed below. `outline` is used only on `neutral`, `surface`, `sunken`, and `accent-tint`, where it reaches 4.7:1 or more in both themes.

| Pair (foreground on background) | Light | Dark |
|---|---|---|
| `primary` on `neutral` | 18.2:1 | 17.2:1 |
| `primary` on `surface` | 16.7:1 | 15.7:1 |
| `primary` on `sunken` | 15.2:1 | 13.2:1 |
| `secondary` on `neutral` | 8.7:1 | 11.3:1 |
| `secondary` on `surface` | 8.0:1 | 10.4:1 |
| `secondary` on `sunken` | 7.3:1 | 8.7:1 |
| `muted` on `neutral` | 6.3:1 | 8.2:1 |
| `muted` on `surface` | 5.8:1 | 7.5:1 |
| `muted` on `sunken` | 5.3:1 | 6.3:1 |
| `primary` on `accent-tint` | 15.5:1 | 13.5:1 |
| `muted` on `accent-tint` | 5.4:1 | 6.4:1 |
| `accent` on `neutral` | 7.3:1 | 8.9:1 |
| `accent` on `surface` | 6.7:1 | 8.1:1 |
| `accent` on `sunken` | 6.1:1 | 6.8:1 |
| `accent` on `accent-tint` | 6.2:1 | 7.0:1 |
| `on-accent` on `accent` | 7.3:1 | 8.9:1 |
| `on-accent` on `accent-pressed` | 9.7:1 | 11.7:1 |
| `outline` on `neutral` | 5.9:1 | 6.1:1 |
| `outline` on `surface` | 5.4:1 | 5.6:1 |
| `outline` on `sunken` | 4.9:1 | 4.7:1 |
| `outline` on `accent-tint` | 5.0:1 | 4.8:1 |

Rules that follow from the table:

- Text on `accent` is always `on-accent`.
- `accent` text is allowed on `neutral`, `surface`, `sunken`, and `accent-tint`.
- `muted` is the lowest-contrast text allowed. Nothing lighter is permitted, including placeholders, disabled labels, and timestamps.
- Text is never placed over photos or video thumbnails without a `neutral` or `dark-neutral` backing panel.
- Disabled controls keep 4.5:1 text. They are shown with a dashed outline, no fill, and a one-line reason ("Choose a file first") instead of faded text.

## Typography

One family: **Inter**, with the platform system font (`system-ui`) as the fallback. Numbers that change (speeds, percentages, countdowns) use tabular figures so they do not jitter.

| Token | Size / line height | Weight | Use |
|---|---|---|---|
| `display` | 32 / 40 | 700 | First-run screen headline only |
| `headline` | 24 / 32 | 700 | One per screen: the headline that says what the screen does |
| `title` | 20 / 28 | 600 | Sheet titles, device names in tiles, section titles |
| `body` | 16 / 24 | 400 | Default text |
| `body-sm` | 14 / 20 | 400 | Metadata, helper text, list secondary lines |
| `label` | 14 / 20 | 600 | Buttons, chips, field labels |
| `caption` | 12 / 16 | 500 | Tab bar labels and badges only |
| `numeric` | 16 / 24 | 500 | Speeds, percentages, times (tabular figures) |

Rules:

- **All text is left-aligned.** This includes headlines, paragraphs, list rows, sheets, dialogs, empty states, and errors. Text blocks are never centered or right-aligned.
- Narrow, fixed-width controls whose only content is one short label (a tab bar item, a numeric badge, a progress percentage inside a ring) may center that label because there is nothing to align it to. Any control wide enough to hold a second word is left-aligned.
- Line length is 40 to 72 characters. In wide layouts, text columns stop at 640 px.
- Text scales with the system font setting up to 200%. Layouts reflow; they do not truncate headlines.
- Truncate only file names, and only in the middle (`Lecture 07 Operating…ystems.mp4`), so the extension stays visible. The full name is available on press-and-hold or hover.
- No all-caps text, no underlines except links, no italics.

## Layout

### Spacing scale (4/8)

| Token | Value | Typical use |
|---|---|---|
| `xs` | 4 px | Icon-to-label gap, tight stacks |
| `sm` | 8 px | Gap between related items, chip gaps |
| `md` | 16 px | Screen margin, card padding, gap between rows of content |
| `lg` | 24 px | Gap between groups, sheet padding |
| `xl` | 32 px | Gap between major sections |
| `xxl` | 48 px | Top of first-run content, minimum touch target |
| `xxxl` | 64 px | Large breaks on desktop |

Rules:

- Only these seven values are used for margins, padding, and gaps.
- Component heights and widths are multiples of 8 (40, 48, 56, 64, 72, 80) except 1 px hairlines and 4 px focus offsets.
- Line heights are multiples of 4 (see Typography).
- Icon sizes are 16, 24, and 32 px.
- If a design needs a value that is not on the scale, change the design, not the scale.

### Grid and breakpoints

| Class | Width | Columns | Margins | Gutter | Navigation |
|---|---|---|---|---|---|
| Compact (phones) | under 600 px | 4 | 16 px | 16 px | Bottom tab bar |
| Medium (large phones, tablets) | 600 to 959 px | 8 | 24 px | 16 px | Navigation rail, 80 px wide |
| Expanded (desktop) | 960 px and above | 12 | 32 px | 24 px | Sidebar, 240 px wide |

- Maximum content width on desktop: 1,120 px, left-aligned in the window, not centered.
- Primary actions on phones sit in the thumb zone at the bottom of the content area, never in the top bar.
- Touch targets are at least 48 by 48 px, with at least 8 px between neighboring targets.

### Structure of a screen

Every screen follows the same vertical order:

1. Top bar (48 px): back button or device name on the left, one icon action on the right.
2. Headline (`headline`), left-aligned, with at most one supporting sentence (`support-text`).
3. Primary content.
4. One primary action (`button-primary`), at most.

## Elevation & Depth

The interface is flat. Depth comes from tone, not shadow.

| Level | Treatment | Used for |
|---|---|---|
| 0 | `neutral` background | Screens |
| 1 | `surface` fill, no shadow | Cards, device tiles, chips |
| 2 | `neutral` fill, shadow `0 8px 24px` at 16% black (light) or a `dark-surface` fill with a 1 px `dark-divider` edge (dark) | Bottom sheets, dialogs, menus |

- Scrim behind sheets and dialogs: 50% black.
- No other shadows. No blur effects. No gradients.

## Shapes

| Token | Value | Used for |
|---|---|---|
| `sm` | 4 px | Focus ring corner, small badges |
| `md` | 8 px | Buttons, inputs, banners |
| `lg` | 16 px | Cards, device tiles, sheets (top corners) |
| `full` | 9999 px | Chips, progress bars, avatars |

Borders are 1 px, in `outline` for controls (inputs, secondary buttons, checkboxes) and `divider` for hairlines. Selected tiles and chips swap the border for an `accent-tint` fill plus a `check` icon.

## Components

### Buttons

| Variant | Appearance | Use |
|---|---|---|
| Primary | `accent` fill, `on-accent` label, 48 px high | The one main action per screen: "Choose files", "Accept", "Download" |
| Secondary | `neutral` fill, 1 px `outline` border, `accent` label | Supporting actions: "Scan QR code", "Pause" |
| Quiet | No fill, no border, `accent` label | Low-emphasis actions: "Cancel", "Clear finished" |

- Labels are verbs or verb phrases: "Choose files", "Send 3 files", "Accept", never "OK" or "Submit".
- Label and icon sit left-aligned inside the button; full-width buttons keep the label at the left with the icon at the right edge.
- Pressed state: `accent-pressed` fill. Focus: 2 px `accent` ring with a 4 px offset (`dark-accent` in dark).
- Maximum one primary button visible per screen.

### Inputs

- 48 px high, 1 px `outline` border, `rounded.md`, 16 px horizontal padding.
- The label is always visible above the field (`label` style). A hint line sits below it (`body-sm`, `muted`). There is no placeholder text; the label and hint carry the meaning.
- Error: the border becomes 2 px `primary`, a `triangle-alert` icon appears with a message that states the fix.

### Device tile (hero component)

```
+--------------------------------------------------------------+
| [laptop]  Maya's MacBook Air                          [send] |
|           Mac · same Wi-Fi · trusted                         |
+--------------------------------------------------------------+
```

- Height 80 px, `surface` fill, `rounded.lg`, 16 px padding.
- Left: device-type icon (24 px). Then name (`title`) and a metadata line (`body-sm`, `muted`): OS, link type, trust state.
- Right: a `send` icon button (48 px target) that starts a send to this device with the already-chosen files.
- Selected: `accent-tint` fill and a `check` icon replacing the send icon.
- Tiles stack vertically with 8 px gaps. With no files chosen, tapping a tile opens the file picker.
- States: default, pressed (`sunken` fill), selected, sending (progress bar replaces the metadata line), unreachable (metadata line states why).

### List item

- 64 px high (72 px when it has a thumbnail), left icon or thumbnail, title (`body`), metadata (`body-sm`, `muted`), optional trailing icon action.
- Hairline `divider` between rows, inset 16 px.
- Swipe actions are never the only path to an action; each has a visible overflow (`ellipsis`) button.

### Chips

- 40 px high, `rounded.full`, `surface` fill, 16 px padding, `label` text.
- Selected: `accent-tint` fill, `accent` text, leading `check` icon.
- Used for library filters (Videos, Music, Images, Documents, Archives, Received) and quality presets.

### Tab bar and navigation

- Compact: bottom bar, 64 px, five items: Nearby, Downloads, Browser, Library, Settings. Icon (24 px) above a `caption` label.
- Active item: `accent` icon and label plus a 4 px `accent` bar at the top edge of the item. Inactive: `secondary`.
- Medium: rail with the same items, labels under icons.
- Expanded: 240 px sidebar, icon left, label right, left-aligned, with a Devices item added.

### Progress

- Bar: 8 px tall, `sunken` track, `accent` fill, `rounded.full`.
- Always paired with text: "412 MB of 662 MB · 38.4 MB/s · 7 s left" (`numeric`).
- Indeterminate state (probing a link) uses a moving 40% segment; with reduced motion it becomes a static bar with the text "Checking the link".

### Sheets and dialogs

- Bottom sheet (phones): `neutral` fill, `rounded.lg` top corners, 24 px padding, a 40 by 4 px drag handle at the top-left. Title (`title`) then content then actions.
- Dialog (desktop): 480 px wide, `rounded.lg`, 24 px padding, title and body left-aligned, actions in a row at the bottom, left-aligned, primary first.
- Both trap focus, close with Escape or the system back gesture, and return focus to the trigger.

### Banners

- `sunken` fill, `rounded.md`, 16 px padding, leading icon (24 px), text (`body-sm`, `primary`), optional quiet-button action on the same banner.
- Used for problems and one-time explanations. Never stacked more than two at a time.

### Toasts

- Short confirmations only ("Copied the link"). `primary` fill with `neutral` text in light, inverted in dark, 4 seconds, with an "Undo" quiet action when the action can be undone.

## Do's and Don'ts

**Do**

- Open every screen with a headline that says what the screen does.
- Keep one primary button per screen, and make it the next step of the hero flow when the screen belongs to it.
- Left-align everything, including empty states and errors.
- Use real names, sizes, and times in every mockup and every empty-state example.
- Show status with an icon and words together.
- Put counts and units next to each other ("662 MB", "38.4 MB/s").
- Test every new color pair in both themes before it ships.

**Don't**

- Don't introduce a second hue, a gradient, or a decorative illustration.
- Don't center text, headlines, or empty states.
- Don't use more than one icon set, or mix outlined and filled icons.
- Don't use spacing values outside the scale (no 10, 12, 20, or 28 px gaps).
- Don't use lorem ipsum, "Item 1", "Device A", "User Name", or "Sample file".
- Don't use placeholder text inside inputs as the only label.
- Don't fade text to show it is disabled or secondary; use `muted`, which already passes 4.5:1.
- Don't write "Oops", "Whoops", "Something went wrong", or exclamation marks.
- Don't promote more than one feature to hero treatment.

## Icons

**One icon set: Lucide** (open source, consistent 24-unit grid, outlined style). On React Native use `lucide-react-native`; on desktop and web use `lucide-react`.

- Sizes: 16 px (inside chips and buttons next to text), 24 px (default), 32 px (empty-state and first-run).
- Stroke: 2 px at every size, using absolute stroke width so it does not scale thin at 16 px.
- Color: `primary` or `secondary` for neutral icons, `accent` for actionable or active icons. Same contrast rule as text (4.5:1).
- Icons never appear alone without an accessible label. Icon-only buttons have a visible tooltip on desktop and a spoken label on mobile.
- No filled variants, no custom icons in v1.

| Purpose | Lucide icon |
|---|---|
| Nearby / send | `send` |
| Scanning | `radar` |
| Scan a code | `scan-line`, `qr-code` |
| Phone, tablet, laptop, desktop, browser guest | `smartphone`, `tablet`, `laptop`, `monitor`, `globe` |
| Download | `download` |
| Paste a link | `link`, `clipboard-paste` |
| Browser | `compass` |
| Library | `folder-open` |
| Settings | `settings` |
| Video, audio, image, document, archive, app | `file-video`, `file-audio`, `image`, `file-text`, `file-archive`, `package` |
| Pause, resume, retry, cancel | `pause`, `play`, `rotate-cw`, `x` |
| Done, selected | `check` |
| Problem, failure | `triangle-alert`, `circle-x` |
| Trusted device | `shield-check` |
| Same Wi-Fi | `wifi` |
| Delete | `trash-2` |
| More actions | `ellipsis` |
| Search | `search` |
| Folder | `folder` |

## Motion

- Durations: 120 ms (press feedback), 200 ms (sheets, selection), 320 ms (screen transitions). Easing: standard ease-out.
- The scanning icon pulses once every 2 seconds while looking for devices. Nothing else loops.
- Progress bars animate their fill but never bounce or overshoot.
- With "reduce motion" on, durations drop to 0, the pulse stops, and indeterminate bars become static with a text description.

## Screens and flows

All sample content below is the content to use in mockups and design reviews. It is deliberately specific.

### Sample content set

| Kind | Examples |
|---|---|
| This device | Kofi's Pixel 8 (Android) |
| Nearby devices | Maya's MacBook Air (Mac), Office PC (Windows 11), Ama's iPhone 15 (iPhone), Nana's Galaxy Tab (Android tablet), Living Room Mini PC (Linux) |
| Files | `Lecture 07 Operating Systems.mp4` (734 MB), `Thesis draft v3.docx` (1.2 MB), `Chapter 4 figures.zip` (48.6 MB), `Defense rehearsal.mp4` (612 MB), `IMG_4821.HEIC` (3.4 MB), `ubuntu-24.04.2-desktop-amd64.iso` (5.9 GB), `Podcast Episode 112.mp3` (58 MB) |
| Speeds and times | 38.4 MB/s, 8.2 MB/s, 7 s left, 2 min left, 21 s total |
| Sources | Direct link to a PDF, a video page, a music page |

### Headlines

Every headline says what the screen does.

| Screen | Headline | Supporting line |
|---|---|---|
| First run | Send files to any nearby device | Works between Android, iPhone, Windows, Mac, and Linux. No account. No internet needed. |
| Nearby (home) | Send files to any nearby device | 3 devices found on this Wi-Fi network |
| Choose files | Choose what to send | Pick files or folders, then pick a device |
| Incoming request | Maya's MacBook Air wants to send you 3 files | 662 MB in total |
| Transfer | Sending 3 files to Maya's MacBook Air | 412 MB of 662 MB |
| Pair with QR | Connect with a QR code | Use this when devices don't appear on their own |
| Downloads | Download files, videos, and music from a link | 2 downloading, 1 paused |
| Choose quality | Choose a quality to download | Rust ownership explained · 18:42 |
| Browser | Find videos and files on any web page | Open a page and tap Download when it appears |
| Library | Find, play, and send your files | 128 files · 14.2 GB |
| Devices | Manage the devices you trust | 3 trusted devices |
| Settings | Settings | (none) |
| Web share page | Download files from Kofi's Pixel 8 | 3 files · 662 MB |

### Flow overview

```
First run -> Nearby (hero) -> Choose files -> [tap device] -> Transfer -> Done -> Nearby
                |                                  ^
                |-> Pair with QR ------------------|
                |-> Downloads -> Choose quality -> Download -> Send to a device -> Transfer
                |-> Library -> select -> Send
```

### 1. First run (compact)

```
Kofi's Pixel 8                                          

Send files to any
nearby device

Works between Android, iPhone, Windows, Mac,
and Linux. No account. No internet needed.

[radar]  Looks for devices on your Wi-Fi network
         and by Bluetooth. Nothing leaves your
         devices.

+--------------------------------------------+
| Find nearby devices                [arrow] |
+--------------------------------------------+
Skip for now
```

- One headline (`display`), one supporting paragraph, one explanation line, one primary button.
- Tapping the button triggers the system permission prompts for local network and Bluetooth, each preceded by the one-line reason above. If a permission is declined, the Nearby screen shows a banner: "Bluetooth is off, so DropGrab can only find devices on the same Wi-Fi network." with a quiet button "Open settings".

### 2. Nearby (home, the hero screen)

```
Kofi's Pixel 8                       [qr-code]
Visible to everyone nearby

Send files to any nearby device
[radar] 3 devices found on this Wi-Fi network

+--------------------------------------------+
| [laptop]  Maya's MacBook Air        [send] |
|           Mac · same Wi-Fi · trusted       |
+--------------------------------------------+
+--------------------------------------------+
| [monitor] Office PC                 [send] |
|           Windows 11 · same Wi-Fi          |
+--------------------------------------------+
+--------------------------------------------+
| [smartphone] Ama's iPhone 15        [send] |
|           iPhone · same Wi-Fi · trusted    |
+--------------------------------------------+

+--------------------------------------------+
| Choose files                    [folder]   |
+--------------------------------------------+
[scan-line] Scan a QR code

Download from a link
Paste a link to a file, video, or song.
+----------------------------------+ +----------+
|                                  | | Download |
+----------------------------------+ +----------+

Recent
[file-video] Lecture 07 Operating Systems.mp4
             Sent to Maya's MacBook Air · 734 MB · 2 min ago
[file-text]  Thesis draft v3.docx
             Received from Ama's iPhone 15 · 1.2 MB · yesterday

[send] Nearby  [download] Downloads  [compass] Browser  [folder-open] Library  [settings] Settings
```

Behavior:

- The device list is the hero: it takes the first 60% of the screen height and updates live. New devices slide in at the top with a 200 ms fade; departed devices are removed after 10 seconds of absence.
- The visibility line ("Visible to everyone nearby") is a quiet button that opens a sheet with three options: Everyone nearby, Trusted devices only, Hidden.
- The QR icon in the top bar shows this device's pairing code (see Pair with QR).
- The primary button "Choose files" is the only primary button on the screen. Choosing files first, then tapping a device, sends immediately after the receiver accepts. Tapping a device first opens the file picker.
- The link field is intentionally small and below the fold of the hero. Its button reads "Download", not "Go", and is a secondary button. Pressing it starts the Downloads flow.
- With no devices found, the hero area shows (left-aligned): "No devices found on this Wi-Fi network." then "Check that both devices are on the same network and that DropGrab is open on the other one." with two buttons: "Scan a QR code" (primary) and "Try again" (secondary).

### 3. Choose what to send (sheet)

```
Choose what to send
Pick files or folders, then pick a device

[image]        Photos and videos
[file-text]    Documents
[folder]       Folders
[clipboard-paste] Text from the clipboard
[folder-open]  Files already in DropGrab

Selected: 0 files
```

- Choosing items shows a selection bar at the bottom: "3 files · 662 MB" with the primary button "Send 3 files".
- After "Send 3 files", the user taps a device on the Nearby screen (or the device tiles appear inside the sheet).

### 4. Incoming request (sheet)

```
Maya's MacBook Air wants to send you 3 files
[shield-check] Trusted device · 662 MB in total

[file-text]    Thesis draft v3.docx            1.2 MB
[file-archive] Chapter 4 figures.zip          48.6 MB
[file-video]   Defense rehearsal.mp4           612 MB

Saving to: Received
[ ] Always accept files from this device

+--------------------------------------------+
| Accept                                     |
+--------------------------------------------+
Decline
```

- Appears over any screen, plus a notification with Accept and Decline actions.
- For an unknown device the badge line reads "New device · Check that the code 7316 also appears on Maya's MacBook Air" and the "Always accept" checkbox is hidden until the code is confirmed.
- Executables and installers get a banner: "This file can install software. Only accept it if you know who sent it."

### 5. Transfer in progress

```
Sending 3 files to Maya's MacBook Air
412 MB of 662 MB · 38.4 MB/s · 7 s left

[==============================------------]

[check]  Thesis draft v3.docx             1.2 MB
[check]  Chapter 4 figures.zip           48.6 MB
[loader] Defense rehearsal.mp4    362 of 612 MB

[pause] Pause        Cancel
```

- Completion replaces the screen content with: "Sent 3 files to Maya's MacBook Air in 21 s" and the buttons "Done" (primary) and "Send more" (secondary).
- Pause changes the label to "Resume" and the status line to "Paused at 62%".
- Failure: "Couldn't finish sending Defense rehearsal.mp4. Maya's MacBook Air went out of range at 362 MB." with "Resume" (primary) and "Cancel" (quiet). Resume continues from 362 MB.

### 6. Connect with a QR code

```
Connect with a QR code
Use this when devices don't appear on their own

[ QR code, 192 by 192 ]
Your code: 482 917
Expires in 1:48

Kofi's Pixel 8 · fingerprint 4F3A 91C2 7D08 B6E1

+--------------------------------------------+
| Scan another device's code                 |
+--------------------------------------------+
```

- After scanning, both screens show a 4-digit check: "Does Maya's MacBook Air show 7316?" with "Yes, they match" (primary) and "No, cancel" (secondary).
- After a match: "Connected to Maya's MacBook Air" and a checkbox "Trust this device for future transfers".

### 7. Downloads

```
Download files, videos, and music from a link
2 downloading, 1 paused

Download from a link
Paste a link to a file, video, or song.
+----------------------------------+ +----------+
|                                  | | Paste    |
+----------------------------------+ +----------+
+--------------------------------------------+
| Download                                   |
+--------------------------------------------+

Downloading
[file-video] Rust ownership explained.mp4
[==================------------]  63%
1080p · 412 MB · 8.2 MB/s · 2 min left        [pause]

[file-audio] Podcast Episode 112.mp3
[========--------------------]  27%
58 MB · 5.4 MB/s · 8 s left                   [pause]

Paused
[package] ubuntu-24.04.2-desktop-amd64.iso
Paused at 41% · 2.4 of 5.9 GB                 [play]

Finished
[file-text] Annual report 2025.pdf · 4.1 MB · today  [send] [ellipsis]
```

- Tabs are replaced by section headings on compact screens to avoid hiding content.
- A copied link triggers a banner at the top: "Download the link you copied? example.org/files/report.pdf" with "Download" and "Dismiss".
- Finished items always show a `send` icon: "Send to a device".
- The remote-download option appears in the overflow menu: "Download on Office PC" (only for trusted devices that allow it).

### 8. Choose a quality

```
Choose a quality to download
Rust ownership explained · 18:42

( ) Best       1080p MP4        412 MB
(•) Balanced   720p MP4         198 MB
( ) Data saver 480p MP4          96 MB
( ) Audio only M4A               17 MB

Save to: Videos

+--------------------------------------------+
| Download 198 MB                            |
+--------------------------------------------+
```

- Presets are chips on compact screens; "More formats" opens the full list of codecs and bitrates.
- DRM-protected: "This video is copy-protected, so DropGrab can't save it." with a single quiet button "Close". No retry.
- Unsupported link: "DropGrab can't find a video or file at this link." with "Open in Browser" and "Try again".

### 9. Browser

```
Find videos and files on any web page
Open a page and tap Download when it appears

+----------------------------------+ [x]
| example.org/lectures             |
+----------------------------------+

[ web page ]

+--------------------------------------------+
| [download] Download · 2 found              |
+--------------------------------------------+
```

- The floating "Download · 2 found" button appears only when media is detected. It is the one primary action in this screen.
- The detected-items sheet lists each item with type, duration or size, and quality, for example "Lecture 07 Operating Systems · 1:12:08 · 1080p · 734 MB".
- Private mode is a setting, shown as a `shield-check` icon in the top bar when on.

### 10. Library

```
Find, play, and send your files
128 files · 14.2 GB

[search] Search your files

( All ) ( Videos ) ( Music ) ( Images ) ( Documents ) ( Archives ) ( Received )

[file-video] Lecture 07 Operating Systems.mp4
             Downloaded · 734 MB · 3 days ago
[file-text]  Thesis draft v3.docx
             Received from Ama's iPhone 15 · 1.2 MB · yesterday
[image]      IMG_4821.HEIC
             Received from Maya's MacBook Air · 3.4 MB · Monday
```

- Long-press (or checkbox on desktop) selects. The selection bar shows "3 selected · 662 MB" with the buttons "Send", "Share", and "Delete".
- Delete asks: "Delete Thesis draft v3.docx? This removes the file from this device." with "Delete" and "Cancel". Multi-delete names the count and total size.
- Empty library: "Nothing here yet. Files you download or receive will appear here." with "Send or receive a file" (primary).

### 11. Devices

```
Manage the devices you trust
3 trusted devices

[laptop]     Maya's MacBook Air             [shield-check]
             Trusted · last seen today, 09:41
             Accept files automatically            (on)
             Allow downloads on this device         (off)

[smartphone] Ama's iPhone 15                [shield-check]
             Trusted · last seen yesterday, 18:20
```

- Each device opens a detail sheet with "Forget this device" and "Block this device".
- Forget confirmation: "Forget Ama's iPhone 15? You'll need to confirm a code the next time you connect." with "Forget" and "Cancel".

### 12. Settings

| Group | Items |
|---|---|
| This device | Device name, Visibility |
| Receiving | Save location, Ask before accepting |
| Downloads | Download folder, Downloads at once, Wi-Fi only, Speed limit |
| Media | Default quality, Audio format |
| Appearance | Theme (System, Light, Dark), Text size |
| Privacy | Clear history, Clear saved logins |
| About | Version, Open source notices, Contact |

Rows use the list-item component. Switches are 48 px targets with a visible On or Off label (the state is never shown by color alone).

### 13. Desktop (expanded layout)

```
+-----------------+-----------------------------------------------------------+
| DropGrab        |  Send files to any nearby device                          |
|                 |  3 devices found on this Wi-Fi network                    |
| [send] Nearby   |                                                           |
| [download]      |  +-----------------------------+  Recent                  |
|   Downloads     |  | Maya's MacBook Air   [send] |  Lecture 07 Operating... |
| [compass]       |  +-----------------------------+  Sent · 734 MB · 2 min   |
|   Browser       |  | Office PC            [send] |                          |
| [folder-open]   |  +-----------------------------+  Thesis draft v3.docx    |
|   Library       |  | Ama's iPhone 15      [send] |  Received · 1.2 MB       |
| [shield-check]  |  +-----------------------------+                          |
|   Devices       |                                                           |
| [settings]      |  Drop files here to send to the selected device           |
|   Settings      |                                                           |
+-----------------+-----------------------------------------------------------+
```

- The device list is the first column and takes 7 of 12 columns; "Recent" takes the remaining 5.
- Dropping files on a tile sends to that device. Dropping a link anywhere on the window starts a download.
- The tray or menu-bar icon shows a badge with the count of incoming requests. Incoming requests appear as a dialog in the window and as a system notification.
- Keyboard: `Ctrl or Cmd + O` choose files, `Ctrl or Cmd + V` paste a link, `Ctrl or Cmd + 1 to 5` switch sections, `Esc` close dialogs.

### 14. Web share page (receiver opens it in a browser)

```
Download files from Kofi's Pixel 8
3 files · 662 MB

Enter the 6-digit code shown on Kofi's Pixel 8
+------------------+
| 5 0 4 1 9 3      |
+------------------+
+--------------------------------------------+
| Show files                                 |
+--------------------------------------------+
Expires in 9:12
```

- After the code is accepted: a file list with a `download` icon per file and a primary button "Download all as .zip (662 MB)".
- The page has no navigation, no scripts from other sites, and follows the same tokens and both themes.
- Certificate warning help (one banner above the form): "Your browser may warn that this connection isn't trusted. Check that the fingerprint 4F3A 91C2 7D08 B6E1 matches the one on Kofi's Pixel 8, then continue."

## Content and copy

### Voice

- Say what happened and what to do next in two short sentences or fewer.
- Use the real name of the device, file, or site. "Maya's MacBook Air", not "the other device".
- Use numbers with units. "412 MB of 662 MB", "7 s left".
- Buttons are verbs: "Accept", "Decline", "Choose files", "Send 3 files", "Download 198 MB", "Forget".
- No exclamation marks, no emoji, no jokes in error states.

### Error copy

| Situation | Copy | Actions |
|---|---|---|
| Device not reachable | Maya's MacBook Air isn't reachable. Both devices need to be on the same Wi-Fi network, or you can connect with a QR code. | "Scan a QR code", "Try again" |
| Receiver declined | Maya's MacBook Air declined the files. | "Send again", "Close" |
| Out of space (receiver) | Maya's MacBook Air doesn't have enough space. It needs 662 MB and has 410 MB free. | "Send fewer files", "Close" |
| Out of space (this device) | Not enough space to download this file. It needs 5.9 GB and 2.1 GB is free. | "Manage storage", "Cancel" |
| Connection lost | The connection dropped at 62%. DropGrab will keep trying for 2 minutes. | "Resume now", "Cancel" |
| Link expired | This download link has expired. Open the page again to get a new one. | "Open in Browser", "Close" |
| Login needed | This file needs you to sign in on the site first. | "Open in Browser", "Cancel" |
| Permission denied | DropGrab can't see nearby devices because Local Network access is off. | "Open settings" |
| Unknown device certificate changed | Maya's MacBook Air has a new security code. Confirm it before sending anything. | "Check the code", "Cancel" |

### Empty states

| Screen | Copy |
|---|---|
| Nearby, no devices | No devices found on this Wi-Fi network. Check that DropGrab is open on the other device. |
| Downloads, nothing yet | No downloads yet. Paste a link above or open the Browser tab to find videos. |
| Library, nothing yet | Nothing here yet. Files you download or receive will appear here. |
| Devices, none trusted | No trusted devices. After your first transfer, you can choose to trust the device. |
| Search, no results | No files match "thesis". Try a shorter name. |

### Notifications

| Event | Title | Body |
|---|---|---|
| Incoming request | Maya's MacBook Air wants to send you 3 files | 662 MB · Accept or decline |
| Transfer finished | Received 3 files | From Maya's MacBook Air · 662 MB |
| Download finished | Download finished | Rust ownership explained.mp4 · 198 MB |
| Download failed | Download stopped | Podcast Episode 112.mp3 · The link expired |
| Remote download request | Office PC asked this device to download a file | Annual report 2025.pdf · 4.1 MB |

With "Hide details on lock screen" on, titles become "Incoming files" and "Download finished" with no names.

## Accessibility

- **Contrast:** 4.5:1 or higher for all text and icons in both themes. The table in the Colors section is the source of truth. Re-run the check whenever a token changes.
- **Targets:** 48 by 48 px minimum, 8 px spacing between targets.
- **Focus:** a visible 2 px accent ring with a 4 px offset on every focusable element. Focus order follows reading order. Sheets and dialogs trap focus and restore it on close.
- **Screen readers:** every icon-only button has a spoken label ("Send to Maya's MacBook Air"). Progress is announced at 25% steps and on completion. New devices are announced politely ("Office PC found").
- **Text size:** supports up to 200%; layouts reflow, no horizontal scrolling for text.
- **Not by color alone:** selection uses a check icon, status uses icon and words, switches show On and Off text.
- **Motion:** respects reduce-motion. Nothing flashes more than 3 times per second (nothing flashes at all).
- **Keyboard (desktop):** every action is reachable by keyboard; shortcuts are listed in a "Keyboard shortcuts" sheet.
- **Language and direction:** all text is externalized; right-to-left layouts mirror the layout and keep text start-aligned, which is the left-aligned rule expressed for right-to-left scripts. Numbers, file names, and fingerprints stay left-to-right.

## Rule compliance

| # | Rule | How this design meets it | Check |
|---|---|---|---|
| 1 | One accent color plus neutrals | `colors` has one chromatic hue (`accent` family) and gray neutrals. Status uses icon and words, not extra hues | Lint `orphaned-tokens`: every color is used by a component. Review: no hue other than the accent appears in any mockup |
| 2 | Headline says what it does | The Headlines table lists a descriptive headline for every screen; "Welcome", "Hello", and brand slogans are banned | Review each screen's first line against the table |
| 3 | One hero feature | Sending to a nearby device is the only hero; it gets 60% of the home screen and the only primary button on it | Check that no other feature has a hero-sized element or its own primary button on the home screen |
| 4 | Left-aligned text, one icon set | All text blocks are left-aligned (one narrow exception for single-label fixed-width controls); Lucide is the only icon set at 2 px stroke | Search for center-aligned styles and non-Lucide icon imports in code review |
| 5 | 4/8 spacing scale | Spacing tokens are 4, 8, 16, 24, 32, 48, 64; component sizes are multiples of 8; line heights are multiples of 4 | Design-token lint and a spacing audit in each build |
| 6 | Contrast of at least 4.5:1 | Every text and icon pair in both themes is at or above 4.5:1 (lowest: `outline` on `sunken` in dark at 4.7:1 and `outline` on `sunken` in light at 4.9:1) | `npx @google/design.md lint` `contrast-ratio` rule for component pairs, plus the measured table above |
| 7 | Real content, no placeholders | Sample content set, headlines, errors, empty states, and notifications use specific names, sizes, and times. Inputs use visible labels, not placeholder text | Review: search mockups for "Lorem", "Item", "Device 1", "User", "Sample" |

Deviations from a strict reading of the rules, stated openly:

1. **Two tones of one accent.** Dark mode uses a lighter tone of the same hue (`dark-accent`) because one value cannot reach 4.5:1 on both white and near-black.
2. **Centered label in fixed-width controls.** Tab bar labels, numeric badges, and a percentage inside a ring may center their single short label.
3. **Decorative hairlines.** `divider` and `dark-divider` are below 4.5:1 by design. They never carry meaning on their own, and every control has an `outline` border that does meet 4.5:1.

## Using this file

- Lint the file (checks token references, section order, and contrast of component pairs):

  ```
  npx @google/design.md lint DESIGN.md
  ```

  Result when this version was written: 0 errors, 0 warnings (24 colors, 8 typography scales, 4 rounding levels, 7 spacing tokens, 44 components).

- Export tokens to code:

  ```
  npx @google/design.md export --format css-tailwind DESIGN.md > theme.css
  npx @google/design.md export --format dtcg DESIGN.md > tokens.json
  ```

- Compare versions of the design system:

  ```
  npx @google/design.md diff DESIGN.md DESIGN-v2.md
  ```

- Implementation notes for the apps: map the tokens to a single theme object consumed by React Native on mobile and by the web UI on desktop. The theme switches between the light and `dark-` values by system setting. No component reads a raw hex value.
- Keep this file named `DESIGN.md` at the root of the design folder so coding agents can find it. Update the tokens first, then the prose.
