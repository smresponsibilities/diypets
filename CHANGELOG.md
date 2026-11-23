# Changelog

## [0.2.0] - 2026-11-23
### Added
- **Native `stdio` MCP Server (`diypets mcp`):** Direct JSON-RPC 2.0 stdin/stdout bridge for Claude Desktop, Cursor, and Claude Code (`"command": "diypets", "args": ["mcp"]`) exposing all 6 tools (`react_pet`, `say`, `drop_ball`, `spawn_pet`, `set_follow_cursor`, `list_herd`).
- **Fullscreen Auto-Hide & Quiet Mode:** Automatically hides pets and pauses physics during fullscreen games, videos, and screen shares, plus a manual **Quiet Mode (Hide Pets)** tray toggle.
- **Pet-to-Pet Herd Interactions:** Pets meeting on the same floor perform friendly greeting hops with pixel hearts or polite bump-turns instead of walking through each other.
- **Cross-Platform Parity (`src/platform.rs`):** Added macOS CoreGraphics global cursor tracking, smooth non-Windows dragging, cross-platform pixel hearts in softbuffer, and non-blocking right-click pet menus (`osascript` / `zenity`).
- **Studio V3 Hardening (`diypets-creator`):** Added **`6. Sandbox`** live physics playground (drag/toss onto a mini window ledge + bouncy ball), **Marquee Select** (`Copy`/`Paste`/`Flip H`/`Del` + `Ctrl+C`/`Ctrl+V`), and an **Unsaved Changes** close guard.

### Changed
- **Faster `cargo install`:** Deduped `toml` (`1.x`) and `base64` (`0.22`) in `Cargo.toml` to trim duplicate crates from the dependency tree.
## [0.1.6] - 2026-11-23
### Added
- **Play Mode (`T-044`):** Drop a bouncy physics pixel ball (`Drop Bouncy Ball` in tray/right-click menu, `diypets ball`, or MCP `drop_ball`) that your herd chases and kicks with floating hearts, plus a **Follow Cursor** toggle.
- **Full 6-Tool MCP & HTTP Agent API (`127.0.0.1:3721`):** Added `say`, `drop_ball`, `set_follow_cursor`, and `list_herd` MCP tools alongside `react_pet` and `spawn_pet`, plus `GET /herd` and CLI shortcuts `diypets say "..."` and `diypets ball`.
- **Per-Pet Herd Sizes & Speeds (`T-032`):** Independent pixel size and walk speed sliders (with Reset) per active pet slot in **Settings -> Active Herd**.
- **Single-Instance Guard:** Launching `diypets` while already running wakes the active pet cleanly instead of spawning duplicate tray icons.
- **Distribution & Automation (`T-017`):** Added Scoop, Homebrew Tap, and Winget manifests under `packaging/`, automatic `.sha256` checksum uploads in Release CI, and an auto-updating README stats workflow.

### Changed
- **Dead-Code Diet (`T-013`):** Removed ~1,600 lines of unused Format v0 modules (`state.rs`, `pack.rs`, `sprite.rs`) for faster `cargo install` builds.
- **Icon & Sprite Polish:** Rebuilt full-bleed edge-to-edge Highland Cow `.ico`/`logo-512.png` for crisp taskbar sizing and cleaned exterior fringe and heavy grey slabs on `polar_bear`.
## [0.1.5] - 2026-11-23
### Added
- **Floating Pixel Speech Bubbles (`src/bubble.rs`):** Crisp click-through speech bubbles float above your pet and track it when triggered via MCP (`/mcp`), HTTP (`/react`), CLI (`diypets react --mood happy --text "Tests passed!"`), or Headpat.
- **Modern Tabbed Settings UI (`diypets-settings`):** Organized into Pet Gallery, Active Herd, and Behavior & AI Agent tabs, with drag-and-drop `.xml`/`.svg` import, live Speech Bubble tester, copyable MCP config, Launch Pet button, and background GitHub release update banner.
- **Studio V3 Hardening (`diypets-creator`):** Live ghost-rigging composite on the Body canvas while dragging joint pins (with `Ctrl+Z` undo), Aseprite-style Space/Middle-mouse pan & Ctrl+Wheel zoom viewport, visual Keyframe Filmstrip with 64x64 thumbnails, and a 5th **Sprite Sheet** tab to slice & export any PNG grid in seconds.
- **Real Vertical Climbing & Window Platform Riding:** Engine evaluates `only="..."` and math `repeat` expressions for OG eSheep parity, plus pets ride moving desktop windows.
- **Mix-and-Match Tray Submenu & No-Cargo Installers:** Choose any installed pet from the tray's `Add Pet to Herd (+1)` submenu; added 1-line `install.ps1` / `install.sh` and embedded the Highland Cow `.ico` into Windows `.exe` binaries.

### Fixed
- **Zero Foot-Skating:** Walk/run frame interval scales proportionally with `walk_speed`.
- **Full 5-Original Parity:** Fixed `dog` & `polar_bear` climb gravity (`G=0`), smooth looping `fall`/`drag`, added `climb` to `goat`, and added `sleep` + `climb` to `stegosaurus`.
## [0.1.4] - 2026-11-22
### Added
- **AI Agent MCP & Local HTTP API:** `diypets` listens on `127.0.0.1:3721` (`/status`, `/react`, `/mcp` JSON-RPC 2.0) plus `diypets react --mood happy|sleep|hop|walk` CLI and drop-in `skills/diypets-creator/SKILL.md`.
- **Headpats & Floating Pixel Hearts:** Rubbing cursor over a pet, clicking Headpat, or sending an agent reaction spawns floating pixel hearts and a happy hop.
- **Direct Right-Click Pet Menu:** Right-click any pet to Headpat, Sleep, Clone (+1), Dismiss, or open Settings/Studio.
- **Studio V3 Ergonomics:** Light/Dark checkerboard toggle, Ctrl+Wheel canvas zoom, paused keyframe pose scrubbing, and Part Z-order (`^`/`v`) reordering.
- **Scaled Frame Cache & Multi-Monitor Left Wall:** Cached scaled ARGB frames for lower idle CPU + full negative-X multi-monitor roaming.

### Fixed
- **True Per-Pixel Click-Through on Windows:** Transparent corners pass clicks through while opaque pixels stay draggable.
- **Original Roster Animation Parity:** Fixed `stegosaurus` (`GroundY` 110 -> 98), `dog` (`GroundY` 103 -> 101), `goat` 0px walk foot drift, and `polar_bear` frame recentering; added crisp transparent `brand/logo-512.png` + `brand/diypets.ico`.
## [0.1.2] - 2026-11-22
### Added
- **Multi-Pet Herd Mode:** Run up to 8 simultaneous pets on your desktop via Settings (`+ Add` / Active Herd bar) or the system tray (`Add Another Pet (+1)` / `Remove Extra Pet (-1)` / `Sleep All`).
- **DIYPets Studio V2 & Agent CLI:** 5 creature archetypes (Quadruped, Biped, Bird, Crawler/Snake, Swimmer/Fish), unlimited extra parts (wings, tails, ears, accessories), 5 auto-swapped face expressions, Custom Keyframe Animations, pure-Rust 2D SVG auto-splitter, and headless `--from-svg` / `--build` / `--template` / `--schema` CLI.
- **Automated Stress Tests:** 40,000-tick 8-pet physics stress test, full bundled roster simulation, concurrent atomic settings R/W test, and malformed SVG fuzz tests.

### Fixed
- **Seamless Silhouette Outlining:** Eliminated cut-out shoulder/hip seams, clipped fur markings inside body silhouettes, and fixed Sit/Sleep/Fall/Climb poses and Surprised eye art.
- **Monitor Shrink Clamp:** Stranded pets now clamp cleanly back onto screen when monitor bounds shrink, and high upward tosses clamp at the top ceiling.
- **Author Attribution & Slim Crate:** Stamped `By SM` onto all 5 originals, rebuilt `catalog.json`, updated yanked `clipboard-win`, and slimmed the `.crate` upload from 7.18 MB to 1.09 MB.
## [0.1.1] - 2026-11-22
### Added
- **DIYPets Studio (`diypets-creator`) Overhaul:** True alpha checkerboard canvas, 3 starter templates (Cat, Buddy, Slime), Shading brush, Recolor swap, 1-Click Auto-Outline, Mirror X drawing, Rect/Oval/Line tools, interactive drag-and-drop joint rigging, always-live preview, 20-frame Contact Sheet inspector, `.diypet.json` save/open, standalone `.xml` sharing, and 9-state desktop export.
- **Lazy On-Demand eSheep Catalog:** 26+ classic eSheep packs load on demand from GitHub into OS user data directories, keeping the crate under 7.2 MB.
- **Start on Login (Autostart):** Native cross-platform autostart toggle in Settings and the System Tray.
- **Import Pet (`.xml`):** One-click local XML pack importer inside Settings.
- **Windows Release UX:** Hidden console window (`windows_subsystem = "windows"`) and crisp 64x64 startup window/tray icons.

### Fixed
- Idle breathing now keeps paws grounded; Fall shows mid-air drop motion & speed lines; Climb renders a vertical window wall with paw-over-paw motion.
- Settings gallery opens instantaneously with crisp 128px previews and two rows (**Made by SM** + **eSheep** with upstream credit).
- Sleep and Climb tray/state fallbacks work cleanly across packs missing optional states.

## [0.1.0] - 2026-11-22
- Initial crate capture on crates.io (`diypets`, `diypets-settings`, `diypets-creator`) with 5 bundled Made by SM originals.