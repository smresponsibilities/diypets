# DIYPets Product & Engineering Roadmap

**Vision:** Make **DIYPets** the #1 open-source, zero-bloat Rust desktop pet engine and creator studio—uniting nostalgic eSheep compatibility, a genuinely fun 2D/SVG pet studio, Multi-Pet Herd Mode, and modern AI coding-agent reactivity.

---

## Pillar 1: Desktop Runtime, Physics & Performance (`diypets`)
**Goal:** Silky 60 FPS movement at <1% idle CPU, native OS feel, zero visual glitches.
- **True Per-Pixel Click-Through (P0):** Transparent sprite corners pass clicks to windows beneath while opaque pixels remain clickable & draggable (stop setting `WS_EX_TRANSPARENT` across the full window).
- **Direct Pet Interaction (P0):** Right-click directly on any pet for quick actions (Headpat, Sleep, Sit, Clone, Dismiss, Settings) without hunting the system tray.
- **Cached High-Quality Sprite Scaling (P1):** Pre-cache scaled frames per `(frame_idx, facing, pet_size)` so walking doesn't re-scale every tick; use Lanczos/bilinear for hi-res >=96px art and nearest-neighbor for retro pixel art.
- **Full Multi-Monitor Support (P1):** Pass both `wall_left` and `wall_right` into `state.tick`, support negative X/Y secondary monitor coordinates, and respect per-monitor DPI scaling.
- **Native App Identity (P1):** Embed `brand/diypets.ico` (Highland Cow) into Windows `.exe` binaries via `build.rs` + clean macOS `.app` bundle icons.
- **Engine Cleanup (P2):** Replace `meval` (`nom v1.2.4` future-incompat warning) with a tiny zero-dep math evaluator; retire duplicate unused Format v0 engine (`state.rs`, `pack.rs`, `sprite.rs`) and scratch binaries.

## Pillar 2: Settings UI & Pack Discovery (`diypets-settings`)
**Goal:** Consumer-grade polish that feels instant, playful, and effortless.
- **Tabbed Modern Dashboard (P1):** Organize into `Gallery` · `Active Herd` · `Behavior & System` · `Community` · `About & Credits`.
- **Animated Hover Previews (P1):** Hovering any pet card animates a 2-frame live bounce/walk preview.
- **Per-Pet Herd Customization (P1):** Independent size & speed sliders per active pet (e.g., big Highland Cow + tiny eSheep + fast Bird).
- **Live Remote Catalog & Drag-and-Drop Import (P1):** Asynchronously refresh `catalog.json` from GitHub with offline fallback; drop any `.xml` or `.svg` file directly onto Settings to install immediately.
- **One-Click "Launch Desktop Pet" & First-Run Welcome (P1):** Detect if `diypets` isn't running and offer a 1-click Start button; show a tiny non-intrusive first-run tip pointing to the system tray & Studio.

## Pillar 3: DIYPets Studio V2 -> V3 (`diypets-creator`)
**Goal:** Turn anyone (or any AI agent) into a desktop pet creator in under 2 minutes.
- **Pro Canvas Ergonomics (P0):** Mouse-wheel zoom, Spacebar/middle-click pan, Light/Dark checkerboard toggle (so black outlines stay visible!), marquee Select/Move/Copy/Paste/Scale, recent color swatches, and brush opacity slider.
- **Sprite Sheet & Animated GIF Importer (P0):** Drop any PNG sprite grid or GIF -> pick `TilesX x TilesY` -> assign Walk/Idle/Sleep/Fall frames -> export playable pet in 10 seconds.
- **Timeline Scrubber & Frame-by-Frame Mode (P1):** Pause & scrub exact keyframes in `3. Custom Anims`, duplicate/reorder frames, adjust easing, and allow hand-drawn pixel overrides for Aseprite-style animators.
- **2-Bone Limb IK & Part Layer Order (P1):** Optional upper/lower leg knee bend for fluid gaits, plus Up/Down Z-order buttons for extra parts.
- **Open & Edit Existing `.xml` Packs (P1):** Load any exported `deskpet.xml` back into Studio to tweak `<GroundY>`, walk speed, metadata, or animations.
- **Smarter 2D SVG Importer (P2):** Support `<linearGradient>`, CSS `<style>` classes, rounded `<rect rx/ry>`, elliptical arcs (`A`), and an interactive part-assignment preview before importing.

## Pillar 4: Bundled Roster Quality ("Highland Cow Gold Standard")
**Goal:** Every bundled **Made by SM** mascot has 0px walk foot drift, crisp alpha edges, and all core behaviors.
- Fix `stegosaurus` (`GroundY` 13px floating gap + add `sleep` and `climb`).
- Fix `goat` (walk frame `2` 7px foot drift + add `climb`).
- Polish `dog` (`GroundY` tightening + non-walk vertical consistency) and `polar_bear` (recenter 13px horizontal shift on non-walk frames).
- Fix `negima` eSheep `walk_x` travel and verify border turnbacks on packs without `climb`.
- Add **Dragon (4-leg + wings + tail)** and **Puppy/Fox** Studio templates.

## Pillar 5: Viral Delight & Developer Companion Features (Market Gaps!)
**Goal:** Features users actually film, share, and star repos for.
- **AI Agent & Dev Companion Mode (`diypets-mcp` + Local HTTP + Speech Bubbles) (P0):**
  - Optional lightweight local server (`127.0.0.1:3721/mcp` & `/react`) + `diypets react --bubble "Tests Passed!" --mood happy`.
  - Ships with `skills/diypets-creator/SKILL.md` so Claude Code, Cursor, Codex, and Gemini CLI auto-know how to draw SVGs, build pets, and trigger reactions on build pass/fail/thinking!
- **Headpats & Pixel Hearts (P1):** Petting your mascot with the cursor spawns floating hearts and switches to the `Happy` expression.
- **Play Mode (Cursor Chase & Tossable Ball/Treat) (P1):** Toggle Neko-style mouse chase or spawn a bouncy pixel ball/treat that your herd chases across the taskbar.
- **Pomodoro / Break Buddy (P2):** Gentle screen tap + speech bubble reminder to stretch/hydrate.
- **Typing Buddy Mode & Cozy Optional SFX (P2):** Opt-in subtle bounce/ear-twitch on typing (BongoCat-style) + muted-by-default sound effects.

## Pillar 6: Cross-Platform Excellence & Zero-Friction Packaging
- **Wayland/GNOME Grace (P1):** Auto-detect Wayland, disable unsupported global window-ledge scanning cleanly with an info tooltip, and support `wlr-layer-shell` on Hyprland/Sway.
- **Package Managers (P1):** Winget & Scoop (Windows), Homebrew Tap (macOS), AUR & AppImage (Linux).
- **Update Notifier (P2):** Non-intrusive "New version available" pill inside Settings checking GitHub Releases once per day.

## Pillar 7: Open-Source Branding & Community Growth
- **Highland Cow Brand Kit:** `brand/social-preview.png` (1280x640), `brand/logo-512.png`, `brand/diypets.ico`.
- **10-Second Hero Demo GIF:** Highland Cow + Multi-Pet Herd + Studio V2 SVG/rigging right below the README hero.
- **Community Pack Hub:** Curated community PR workflow + auto-generated `catalog.json` GitHub Action when a new pet pack PR merges!

---

## Release Milestones
- **`v0.1.3` (Polish & UX Fixes):** Per-pixel click-through, direct pet right-click menu, Highland Cow `.exe` icon, frame scale cache, `stegosaurus`/`goat` animation fixes, `meval` removal.
- **`v0.2.0` (Viral Dev & Delight Release):** Local MCP/HTTP agent server + Speech Bubbles, Headpats & Hearts, Sprite Sheet/GIF Importer, Studio Pro Canvas (pan/zoom/light checkerboard/marquee), Tabbed Settings + animated hover cards.
- **`v0.3.0` (Ecosystem Release):** 2-bone IK + Frame-by-frame Studio mode, Per-pet Herd sizes, Play Mode (ball/cursor chase), Winget/Scoop/Homebrew/AppImage, Community Pack Registry.