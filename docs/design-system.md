# KPIO Browser Design System

Version 1.0.0 | 2026-01-30

## Overview

The KPIO Design System is a unified design system for the KPIO Browser OS. It provides a modern, accessible UI/UX and ensures a consistent user experience.

## Design Principles

### 1. Simplicity
- Remove unnecessary elements
- Clear visual hierarchy
- Intuitive interactions

### 2. Accessibility
- Compliant with WCAG 2.1 AA standards
- Keyboard navigation support
- Screen reader compatibility
- Sufficient color contrast

### 3. Responsiveness
- Fluid layouts
- Support for various resolutions
- Touch and mouse interactions

### 4. Consistency
- Unified components
- Predictable behavior
- Reusable patterns

---

## Color Palette

### Primary (Brand Colors)
| Token | Hex | Usage |
|-------|-----|-------|
| Primary-50 | #EFF6FF | Background tint |
| Primary-100 | #DBEAFE | Hover background |
| Primary-200 | #BFDBFE | Active background |
| Primary-500 | #3B82F6 | Primary buttons, links |
| Primary-600 | #2563EB | Hover state |
| Primary-700 | #1D4ED8 | Active/click state |

### Neutral (Grayscale)
| Token | Hex | Usage |
|-------|-----|-------|
| Gray-50 | #F9FAFB | Background |
| Gray-100 | #F3F4F6 | Card background |
| Gray-200 | #E5E7EB | Borders |
| Gray-500 | #6B7280 | Secondary text |
| Gray-900 | #111827 | Primary text |

### Semantic (Meaning Colors)
| Status | Color | Usage |
|--------|-------|-------|
| Success | #22C55E | Completion, success |
| Warning | #F59E0B | Warning |
| Error | #EF4444 | Error, danger |
| Info | #3B82F6 | Information |

---

## Typography

### Font Stack
```
Primary: Pretendard, -apple-system, BlinkMacSystemFont, sans-serif
Mono: JetBrains Mono, Consolas, monospace
```

### Text Scale
| Name | Size | Line Height | Usage |
|------|------|-------------|-------|
| Display LG | 48px | 56px | Large headlines |
| Display MD | 36px | 44px | Medium headlines |
| Heading 1 | 24px | 32px | Page titles |
| Heading 2 | 20px | 28px | Section titles |
| Heading 3 | 18px | 24px | Card titles |
| Body LG | 16px | 24px | Body text (large) |
| Body MD | 14px | 20px | Default body text |
| Body SM | 12px | 16px | Small body text |
| Label | 12px | 16px | Labels, tags |
| Caption | 11px | 14px | Captions, hints |

---

## Spacing System

8px-based grid system:

| Token | Value | Usage |
|-------|-------|-------|
| xs | 4px | Minimum spacing |
| sm | 8px | Inline element spacing |
| md | 12px | Component internal |
| lg | 16px | Component spacing |
| xl | 24px | Section spacing |
| xxl | 32px | Large section |
| xxxl | 48px | Page padding |

---

## Components

### Button

```
┌────────────────────┐
│  [Icon]  Label     │ ← 40px height (Medium)
└────────────────────┘
```

**Variants:**
- **Primary**: Filled blue background
- **Secondary**: Border only button
- **Ghost**: No background, shows on hover
- **Danger**: Red, for dangerous actions like delete

**Sizes:**
- XSmall: 24px
- Small: 32px
- Medium: 40px (default)
- Large: 48px
- XLarge: 56px

### Input

```
┌─────────────────────────────────┐
│ Label                           │
├─────────────────────────────────┤
│ Placeholder...             [x]  │ ← 40px height
├─────────────────────────────────┤
│ Helper text or error message    │
└─────────────────────────────────┘
```

### Card

```
┌─────────────────────────────────┐
│                                 │
│   Title                         │
│   Subtitle                      │
│                                 │
│   Content area                  │
│                                 │
│                     [Actions]   │
└─────────────────────────────────┘
    ↑ 8px radius, shadow
```

### Tabs

```
┌────┬────┬────┬────────────────────┬───┐
│ ⊕  │ 🔵 │ 📄 │ Tab Title...      │ x │
└────┴────┴────┴────────────────────┴───┘
  │     │    │           │           │
  │     │    │           │           └─ Close button
  │     │    │           └─ Title (max 240px)
  │     │    └─ Favicon
  │     └─ Loading indicator
  └─ New tab button
```

---

## Layout

### Browser Chrome

```
┌─────────────────────────────────────────────────────────────┐
│ [←][→][↻][🏠]  │ 🔒 example.com                    │ [⋮]   │ ← Tab bar + Toolbar
├─────────────────────────────────────────────────────────────┤
│ [Bookmark Bar]                                               │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│                                                             │
│                    Content Area                             │
│                                                             │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│ Status: Ready                                   Zoom: 100%  │ ← Status bar
└─────────────────────────────────────────────────────────────┘
```

### Flex Layout

```rust
Flex::row()
    .gap(spacing::MD)
    .justify(JustifyContent::SpaceBetween)
    .align(AlignItems::Center)
```

### Grid Layout

```rust
Grid::new(3)
    .gap(spacing::LG)
    .padding(EdgeInsets::all(spacing::XL))
```

---

## Themes

### Light Theme
- Background: Gray-50
- Surface: White
- Text: Gray-900
- Border: Gray-200

### Dark Theme
- Background: Gray-950
- Surface: Gray-900
- Text: Gray-50
- Border: Gray-700

### Theme Switching
```rust
let theme = match preference {
    ThemePreference::Light => Theme::light(),
    ThemePreference::Dark => Theme::dark(),
    ThemePreference::System => detect_system_theme(),
};
```

---

## Animation

### Duration
| Token | Value | Usage |
|-------|-------|-------|
| Instant | 0ms | Immediate |
| Fast | 100ms | Micro interactions |
| Normal | 200ms | Standard transitions |
| Slow | 300ms | Modals, panels |
| Slower | 500ms | Page transitions |

### Easing
- `ease-out`: Element appearance
- `ease-in`: Element disappearance
- `ease-in-out`: State changes
- `bounce-out`: Emphasis effects

### Accessibility Considerations
- Support for `prefers-reduced-motion`
- Immediate transition when motion is reduced

---

## Icons

### Sizes
- 16px: Inline, inside buttons
- 20px: Default size
- 24px: Toolbar, navigation
- 32px: Empty states, emphasis

### Style
- Stroke width: 2px
- Round line cap
- Round line join

### Categories
- **Navigation**: Arrows, home, refresh
- **Browser**: Tabs, bookmarks, downloads
- **Actions**: Add, delete, edit
- **UI**: Settings, user, notifications

---

## Responsive Breakpoints

| Name | Width | Usage |
|------|-------|-------|
| Mobile | < 640px | Smartphones |
| Tablet | 640px - 1024px | Tablets |
| Desktop | 1024px - 1440px | Desktops |
| Wide | > 1440px | Wide monitors |

---

## Accessibility Guidelines

### Color Contrast
- Normal text: 4.5:1 or higher
- Large text: 3:1 or higher
- UI components: 3:1 or higher

### Focus Indicators
- Focus ring on all interactive elements
- Logical keyboard navigation order
- Skip link provided

### ARIA
- Appropriate role attributes
- Required aria-label provided
- Live regions usage

---

## File Structure

```
design/
├── mod.rs          # Module entry point
├── tokens.rs       # Design tokens (colors, spacing, typography)
├── theme.rs        # Light/dark themes
├── components.rs   # Basic UI components
├── layout.rs       # Layout system
├── icons.rs        # Icon definitions
├── animation.rs    # Animation system
├── browser.rs      # Browser chrome components
├── dialogs.rs      # Dialogs, toasts
└── pages.rs        # Page components
```

---

## Usage Examples

### Creating a Button
```rust
use kpio_browser::design::*;

let button = Button::new("Save")
    .variant(ButtonVariant::Primary)
    .size(Size::Medium)
    .icon("save");
```

### Applying a Theme
```rust
let design = DesignSystem::new()
    .with_theme(Theme::dark())
    .with_scale(1.0);
```

### Layout Configuration
```rust
let layout = Flex::row()
    .gap(spacing::MD)
    .justify(JustifyContent::SpaceBetween)
    .padding(EdgeInsets::all(spacing::LG));
```

---

## Changelog

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-30 | Initial design system release |
