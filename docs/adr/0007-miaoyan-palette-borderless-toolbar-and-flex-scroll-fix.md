# ADR 0007: MiaoYan Design Palette, Borderless Toolbar, and Flex Scroll Constraints

## Status
Accepted

## Context
1. User testing revealed that documents could not be scrolled vertically in the reading view or editor, and clicking Split mode caused the rendered Markdown preview pane to disappear.
2. The user requested adopting the clean, airy design language of **妙言 (MiaoYan)** (pure white background, delicate hairline dividers, fresh leaf-green accent) as the new application default, with the previous Claude Warm palette retained as an alternate choice.
3. The topbar icons currently featured unnecessary square border outlines, cluttering the chrome.
4. The macOS dock icon appeared awkwardly with nested rounded squares instead of a clean gradient-filled logo mark.

## Decision
1. **Flexbox Scroll & Split Constraint Resolution**:
   - In Taffy/CSS flexbox, children with `flex_1()` default to `min-height: auto` (`min-content`). When containing long documents, the scroll container expands to the full content height, resulting in `scroll_max = 0` (no scroll range). Explicitly applying `.min_h(px(0.))` forces the container to shrink to the viewport bounds, unlocking fluid 120fps scrolling.
   - In Split mode, the editor column containing long text lines expanded horizontally past the window edge, pushing the preview pane off-screen. Adding `.min_w(px(0.))` and `.overflow_hidden()` on both columns guarantees strictly proportional, responsive split layouts.
2. **MiaoYan Default Palette**:
   - Design `miaoyan` as the default palette: clean `#FFFFFF` canvas, ultra-light `#ECECEC` hairline dividers, high-contrast `#1E1E1E` typography, and fresh `#32965D` leaf-green accent.
3. **Borderless Icon Chrome**:
   - Remove outer bounding boxes (`border_1()`) from all idle topbar icon buttons. Use borderless icons with soft hover pills (`bg(theme.surface)`), maintaining a quiet, distraction-free reading atmosphere.
4. **App Icon Redesign**:
   - Redesign `AppIcon.icns` with a clean pure white squircle canvas and a large, centered geometric Z logo filled with a warm terracotta-to-golden-amber linear gradient.

## Consequences
- Completely resolves vertical scrolling and split-mode preview rendering bugs.
- Elevates visual design to a native, modern, distraction-free macOS writing experience.
