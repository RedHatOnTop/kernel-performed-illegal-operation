## Goal
- Make English the default documentation language and convert all Markdown files containing Korean text.
- Keep the docs folder easy to navigate and avoid breaking existing links.

## Scope (Files Detected With Korean Text)
- docs/servo-integration-plan.md
- docs/architecture/phase2-implementation-checklist.md
- docs/architecture/phase2-servo-integration.md
- docs/architecture/syscall-design.md
- docs/design/PHASE_0_FOUNDATION.md
- docs/design/PHASE_1_CORE.md
- docs/design/PHASE_1_EXECUTION_PLAN.md
- docs/design/PHASE_2_USERSPACE.md
- docs/design/PHASE_3_GRAPHICS.md
- docs/design/PHASE_4_POLISH.md
- docs/guides/LOCAL_DEVELOPMENT.md

## Docs Structure Changes
- Keep the current file paths as the canonical English docs (so existing references keep working).
- Add a language archive folder:
  - Create docs/ko/ and copy the current Korean originals into mirrored paths (e.g., docs/ko/design/PHASE_1_CORE.md).
- Add a top-level docs index:
  - Create docs/README.md with a clean table of contents to Architecture / Design / Guides / Integration docs.
  - Include a short note pointing to docs/ko/ for archived Korean originals.

## Translation Rules (Consistency)
- Translate headings, paragraphs, tables, checklist items, and inline explanatory text.
- Preserve code blocks exactly, except translating human-language comments/docstrings inside fenced blocks when they are explanatory (no identifier/keyword changes).
- Keep terminology consistent across docs (KPIO, kernel, user space, syscall, WASI, no_std, etc.).

## Implementation Steps
1. Create docs/ko/ and copy the 11 Korean-source files into it (original content preserved).
2. Translate the 11 canonical files in-place to English, preserving Markdown structure.
3. Fix cross-links:
   - Update any references that point to moved/archived Korean versions (only one known reference today in PHASE_1_EXECUTION_PLAN.md).
4. Add docs/README.md as an entry point and ensure it links to the translated docs.

## Verification
- Re-scan docs/**/*.md for Hangul to confirm the canonical docs no longer contain Korean.
- Re-scan for broken internal Markdown links (at minimum: ensure all referenced .md paths exist).
- Spot-check rendering-sensitive sections (tables, fenced blocks, diagrams).

## Notes / Risk Controls
- This approach avoids disruptive renames/moves while still organizing by language.
- If you later want fully normalized naming (lowercase, kebab-case), we can do that as a separate pass after translation to keep this change reviewable.