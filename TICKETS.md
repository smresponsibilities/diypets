# DIYPets Engineering Roadmap & Tickets

## P0 — Immediate Quality, UX Feedback & Polish
- [x] **T-001: Goat Eye & Horn-Gap Cleanup + Automated GIF Visual Validation**
  - Compact 4x6 dark pupils + 2x2 catchlights on original 56-frame Goat; automated `scripts/validate_sprites.py` + CI artifact workflow.
- [ ] **T-002: Action Feedback Toasts & Scratch-Style Beginner Tips (`src/ui_toast.rs` -> `diypets-settings` & `diypets-creator`)** *(IN PROGRESS: shared `ToastQueue` + `tip_banner` landed; wiring all GUI buttons next)*
  - Every button click (`Save`, `Apply`, `Export`, `Auto-Outline`, `Duplicate Frame`, `Reset`, `Install Pack`) triggers an immediate corner confirmation toast + contextual beginner tips so users never feel overwhelmed.
- [x] **T-003: Win32 Layered Visibility Regression Suite, Rotating Logger & `diypets doctor`**
  - Unit tests for `should_auto_hide_fullscreen` and `sanitize_layered_window_styles`, 7-day rotating logger (`src/logging.rs`), `build.rs` fingerprint metadata, and `diypets doctor`.

## P1 — Scratch-Simple Studio, JuiceFX Auto-Animator & 8-Bit Ingestion (`v0.4.0` / Studio V5)
- [ ] **T-004: Intelligent 8-Bit Sprite Sheet Importer (Auto-Detect + User-Guided Fallback)**
  - Ingest any PNG, GIF, JPG, Aseprite/TexturePacker JSON, or packed RPG/SNES sheet: corner chroma-key -> native pixel-block scale detection -> 3 slicing modes (**Auto Grid Gutters**, **Connected-Components Auto-Box**, **Draggable Manual Grid/Box Fallback**) -> locked bottom-center foot anchor -> auto state bucketing (`Idle`, `Walk`, `Sleep`, `Fall`, `Drag`).
- [ ] **T-005: Scratch-Style "Costumes" Flipbook & JuiceFX 1-Click Auto-Animator**
  - Visual numbered **Costumes** strip with visible **Foot Crosshair**, 1-click **Duplicate & Tweak**, plus JuiceFX/PixelOver-style procedural presets (**Auto-Walk Bounce**, **Idle Breathing**, **Tail/Ear Wave**, **Squash Hop**, **Smear**) so a single static sprite animates in 30 seconds.
- [ ] **T-006: True 8-Bit Pixel Editor Core (`diypets-creator`)**
  - Native low-res grid selector (`16x16`, `24x24`, `32x32`, `48x48`, `64x64`) + floating **1:1 Taskbar Live Preview**, **Indexed Palette Mode** (PICO-8, Endesga-32, Game Boy, NES live recoloring), **Pixel-Perfect Brush** (L-double removal), and **2x2 / 4x4 Ordered Bayer Dithering**.
- [ ] **T-007: 1-Click Shareable GIF Exporter & Direct `.aseprite` Importer**
  - Export any animation loop directly to a transparent or themed `.gif` inside Studio for instant posting on X/Reddit/Discord; import `.ase`/`.aseprite` files with automatic tag mapping.
- [ ] **T-008: Real Photo -> Desktop Pet ("Snap Your Real Pet")**
  - Lazy-download `u2netp.onnx` (4.6 MB) via `ort` for local foreground cutout -> 12-16 color 8-bit pixelizer + Auto-Outline -> geometric auto-rigging into `deskpet.xml`.

## P2 — Coding-Agent Ecosystem, IDE Awareness & Viral Reactivity (`v0.5.0`)
- [ ] **T-009: 1-Click Coding-Agent Hooks (`diypets hooks install claude-code / cursor / codex`)**
  - Zero-config hooks so pets automatically show *Thinking*, *Build Passed!*, *Tests Failed*, or *Token Burst* reactions during Claude Code, Cursor, and Codex runs (beating Electron rivals at ~7.8 MB RAM).
- [ ] **T-010: Keystroke & Click Reflex Mode (BongoCat-Inspired)**
  - Privacy-safe global input-cadence counter (never logs keys) so pets tap paws or cheer during fast coding bursts.
- [ ] **T-011: CPU & Build Activity Sprint Mode (RunCat-Inspired)**
  - Optional CPU load binding where pets jog/sprint during heavy compiles and curl up to nap when idle.
- [ ] **T-012: Audio-Reactive Beat Bounce Mode**
  - Optional WASAPI / CoreAudio / PulseAudio level tap so pets bob to music with floating pixel music notes.
- [ ] **T-013: Pomodoro / Focus Companion Mode**
  - Built-in tray timer: pets nap/read quietly during 25m focus blocks and hop with a break reminder at 0:00.
- [ ] **T-014: Collectible Hats, Accessories & Decoupled Eye-Tracking/Blinking**
  - Independent `face_blink` timer, subtle cursor pupil tracking, and equipable pixel hats (party hat, headphones, wizard hat, scarf).

## P3 — State-of-the-Art Platform, Release Engineering & Community (`v0.6.0`)
- [ ] **T-015: `cargo-dist` Prebuilt Installers & Custom Build Craftsmanship**
  - Automated MSI/DMG/AppImage installers, release notes, Homebrew tap sync, and polished CLI progress/diagnostics.
- [ ] **T-016: Community Pet Registry CLI (`diypets install <pet>`)**
  - Curated GitHub registry index for 1-command install and sharing of community-created pets.
- [ ] **T-017: Windows UIA Internal IDE Panel Walking (Socrates-The-Cat-Inspired)**
  - Optional Windows UI Automation support so pets can perch on internal VS Code / terminal split panels and fall naturally when panes resize.
- [ ] **T-018: Multi-Monitor Per-Screen Herd Routing**
  - Pin specific pets to primary vs secondary monitors or allow seamless walking across multi-monitor seams.
- [ ] **T-019: Good First Issue Starter Pack (GitHub Issues Sync)**
  - Publish 5 curated `good first issue` and 4 `help wanted` medium issues on GitHub linked to `CONTRIBUTING.md`.
- [ ] **T-020: Shareable "My Desktop Herd" Snapshot Card**
  - 1-click export of a crisp PNG/GIF card showing your current herd + stats for sharing on social feeds.
