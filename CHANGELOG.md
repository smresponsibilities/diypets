# Changelog

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