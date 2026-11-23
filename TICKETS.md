# DIYPets Engineering Roadmap & Tickets

## P0 — Stability, Art & UX Feedback
- [x] **T-001: Original 56-Frame Goat Cleanup & Automated GIF Visual Validation**
  - *Reference:* Automated contact sheet & animated GIF visual regression (`scripts/validate_sprites.py`, `.github/workflows/visual-validation.yml`).
  - *Status:* Landed compact pupil cleanup + CI artifact pipeline; follow-up hand-retouch tracked in T-001b if walk frames need bespoke 1px pixel art polish.
- [x] **T-002: Instant Action Feedback Toasts & Beginner Tips (`src/ui_toast.rs`)**
  - *Reference:* Scratch editor immediate visual confirmation & non-intrusive guidance so users never guess whether a button worked.
  - *Status:* Landed shared `ToastQueue` + `tip_banner` wired across `docs/index.html`, `diypets-settings`, and `diypets-creator`.
- [x] **T-003: Win32 Layered Visibility Regression Suite, Rotating Logger & `diypets doctor`**
  - *Status:* Unit tests for `should_auto_hide_fullscreen` and `sanitize_layered_window_styles`, 7-day rotating logger (`src/logging.rs`), `build.rs` fingerprint metadata, and `diypets doctor`.
- [x] **T-003b: True Dedicated Climb vs Rotated-Walk Fallback on Web Sandbox**
  - *Status:* Exported 9-frame strips `[idle, walk0..3, climb0..3]` + `climb_sheet.png` audit; uses real climb frames where authored and properly oriented 90-degree wall-grip walk otherwise.

## P1 — Scratch-Simple Studio, Auto-Animation & 8-Bit Ingestion (`v0.4.0` / Studio V5)
- [ ] **T-004: Intelligent 8-Bit Sprite Sheet Importer (Auto-Detect + User-Guided Fallback)**
  - *Reference:* Aseprite / TexturePacker / Piskel sprite-sheet slicing.
  - *Scope:* Ingest any PNG, GIF, JPG, Aseprite JSON, or packed RPG/SNES sheet -> corner chroma-key background removal -> native pixel-block scale detection (strips blurry AI mixels) -> 3 slicing modes (**Auto Grid Gutters**, **Connected-Components Auto-Box**, **Draggable Manual Grid/Box Fallback**) -> locked bottom-center foot anchor -> auto state bucketing (`Idle`, `Walk`, `Climb`, `Sleep`, `Fall`, `Drag`).
- [ ] **T-005: Scratch-Style "Costumes" Flipbook & JuiceFX 1-Click Auto-Animator**
  - *Reference:* **Scratch Costume Editor** (numbered flipbook, visible foot/center crosshair, Duplicate & Tweak) + **JuiceFX / PixelOver** (procedural secondary motion from a single static frame).
  - *Scope:* Add a beginner **Costumes** view with visible foot crosshair + 1-click procedural generators (**Auto-Walk Bounce**, **Idle Breathing**, **Tail/Ear Wave**, **Squash Hop**, **Motion Smear**).
- [ ] **T-006: True 8-Bit Pixel Editor Core (`diypets-creator`)**
  - *Reference:* **Aseprite** & **Pixelorama**.
  - *Scope:* Native low-res grid selector (`16x16`, `24x24`, `32x32`, `48x48`, `64x64`) with floating **1:1 Taskbar Live Preview**, **Indexed Palette Mode** (PICO-8, Endesga-32, Game Boy, NES live swatch recoloring), **Pixel-Perfect Brush** (L-double removal), and **2x2 / 4x4 Ordered Bayer Dithering**.
- [ ] **T-007: 1-Click Shareable GIF Exporter & Direct `.aseprite` Importer**
  - *Reference:* **JuiceFX / Aseprite** instant share loop.
  - *Scope:* Export any pet animation loop directly to a transparent or themed `.gif` inside Studio for posting on X/Reddit/Discord; import `.ase`/`.aseprite` files with automatic tag mapping.
- [ ] **T-008: Real Photo -> Desktop Pet ("Snap Your Real Pet")**
  - *Reference:* Local ONNX foreground matting (`u2netp.onnx`, 4.6 MB lazy-downloaded on first use).
  - *Scope:* Offline cutout via `ort` -> 12-16 color 8-bit pixelizer + 1-click Auto-Outline -> geometric auto-rigging into `deskpet.xml`.

## P2 — Coding-Agent Ecosystem, IDE Awareness & Viral Reactivity (`v0.5.0`)
- [ ] **T-009: 1-Click Coding-Agent Hooks (`diypets hooks install claude-code / cursor / codex`)**
  - *Reference:* **clawd-on-desk** & **OpenPets** (real-time desktop status for coding agents, delivered at ~7.8 MB RAM instead of Electron).
  - *Scope:* Auto-wire agent lifecycle hooks so pets show *Thinking*, *Build Passed*, *Tests Failed*, and *Token Burst* states without manual CLI calls.
- [ ] **T-010: Keystroke & Click Reflex Mode**
  - *Reference:* **BongoCat** (Rust/Tauri).
  - *Scope:* Privacy-safe global input-cadence counter (counts tap rate only, never logs keys) so pets tap paws or cheer during coding bursts.
- [ ] **T-011: CPU & Build Activity Sprint Mode**
  - *Reference:* **RunCat 365**.
  - *Scope:* Optional CPU load binding where pets sprint faster during heavy compiles and nap when idle.
- [ ] **T-012: Audio-Reactive Beat Bounce Mode**
  - *Scope:* Optional WASAPI / CoreAudio / PulseAudio loopback level tap so pets bob to music with floating pixel notes.
- [ ] **T-013: Pomodoro / Focus Companion Mode**
  - *Scope:* Built-in tray timer: pets nap/read quietly during 25m focus blocks and hop with a break reminder at 0:00.
- [ ] **T-014: Collectible Hats, Accessories & Decoupled Eye-Tracking/Blinking**
  - *Reference:* **BongoCat / VPet** cosmetics.
  - *Scope:* Independent `face_blink` timer, subtle cursor pupil tracking, and equipable pixel hats (party hat, headphones, wizard hat, scarf).

## P3 — State-of-the-Art Platform, Release Engineering & Community (`v0.6.0`)
- [ ] **T-015: `cargo-dist` Prebuilt Installers & Custom Build Craftsmanship**
  - *Scope:* Automated MSI/DMG/AppImage installers, release notes, Homebrew tap sync, and custom build/doctor diagnostics.
- [ ] **T-016: Community Pet Registry CLI (`diypets install <pet>`)**
  - *Scope:* Curated GitHub registry index for 1-command install and sharing of community-created pets.
- [ ] **T-017: Windows UIA Internal IDE Panel Walking**
  - *Reference:* **Socrates-The-Cat** (Windows UI Automation panel detection).
  - *Scope:* Optional UIA mode so pets walk on internal VS Code / terminal split dividers and fall naturally when panes resize.
- [ ] **T-018: Multi-Monitor Per-Screen Herd Routing**
  - *Scope:* Pin specific pets to primary vs secondary monitors or let pets walk across multi-monitor seams.
- [ ] **T-019: Publish Starter Issues Pack on GitHub (`good first issue` + `help wanted`)**
  - *Scope:* Create 5 scoped starter issues and 4 medium feature issues on GitHub linked to `CONTRIBUTING.md`.
- [ ] **T-020: Shareable "My Desktop Herd" Snapshot Card**
  - *Scope:* 1-click PNG/GIF snapshot export showing your active herd for easy sharing on X, Reddit, and Discord.
