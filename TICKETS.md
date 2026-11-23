# DIYPets Engineering Tickets

## P0 — Immediate Quality & Stability
- [x] **T-001: Redraw Goat Mascot Properly (`assets/goat/deskpet.xml`)**
  - Stop patching bad raster art with pixel heuristics. Build a clean native 32x32 pixel-art Goat rig matching Highland Cow & Dog (symmetric round dark eyes with 2x2 catchlights, clean 1px `#181216` outline, curved horns, hooves) and export all animation states + `catalog_icons/goat.png` + `docs/sprites/goat.png` + `docs/og-image.png`.
- [x] **T-002: Calibrate True Snappy Default Speed & Size (`src/settings.rs` + `src/deskpet_state.rs`)**
  - Fix slow-motion walk cadence and velocity curve (`walk_speed` range & frame-interval scaling), align `DEFAULT_PET_SIZE` / `DEFAULT_WALK_SPEED`, and update unit tests (`defaults_are_inside_supported_ranges`, `saved_file_is_plain_toml_with_comment`).
- [x] **T-003: Win32 Layered Visibility & Fullscreen Auto-Hide Regression Suite**
  - Unit tests for `should_auto_hide_fullscreen` and `sanitize_layered_window_styles` + rotating 7-day file logger (`src/logging.rs`).

## P1 — Pro 8-Bit Editor & Intelligent Sprite Ingestion (`v0.4.0` / Studio V5)
- [ ] **T-004: Intelligent 8-Bit Sprite Sheet Importer (Auto-Detect + User-Guided Fallback)**
  - Ingest any PNG, GIF, JPG, Aseprite/TexturePacker JSON, or packed RPG/SNES sprite sheet.
  - Auto-pipeline: chroma-key solid background removal -> native pixel-grid scale detection (removes blurry AI mixels/anti-aliasing) -> 3 slicing modes (**Auto Grid Gutters**, **Connected-Components Auto-Box**, and **User-Guided Draggable Grid/Box Fallback**) -> bottom-center foot-baseline alignment -> automatic state bucketing (`Walk`, `Idle`, `Sleep`, `Fall`, `Drag`) with manual click-to-reassign timeline.
- [ ] **T-005: True 8-Bit Pixel Editor Core (`diypets-creator`)**
  - **Native Grid Canvas Selector:** Draw at authentic 16x16 / 24x24 / 32x32 / 48x48 / 64x64 grids (integer-upscaled cleanly on export) with a floating **1:1 Taskbar-Size Live Preview** window.
  - **Indexed Palette Mode & Lospec Presets:** 16/32-color indexed swatches (PICO-8, Endesga-32, Game Boy, NES) where editing a swatch live-recolors every frame + 5-step hue-shifted ramp generator.
  - **Pixel-Perfect Brush & Ordered Dithering:** Aseprite-style L-double removal on 1px curves + 2x2 / 4x4 Bayer dithering brush.
- [ ] **T-006: Dope Sheet Timing, Pose-to-Pose Inbetweening & 1-Click GIF / `.aseprite` Support**
  - Per-frame `ms` durations + easing curves (`Linear`, `Ease-In-Out`, `Bounce`), keyframe auto-inbetweening, direct `.ase`/`.aseprite` file import, and 1-click transparent GIF export.
- [ ] **T-007: Real Photo -> Desktop Pet ("Snap Your Real Pet")**
  - Lazy-download `u2netp.onnx` (4.6 MB) on first use via `ort` for offline foreground cutout -> k-means 12–16 color 8-bit pixelizer + Auto-Outline -> geometric auto-rigging into `deskpet.xml`.

## P2 — Desktop Delight & Ecosystem (`v0.5.0`)
- [ ] **T-008: Audio-Reactive Beat Bounce Mode**
  - Optional WASAPI / CoreAudio / PulseAudio level tap so pets bob to music with floating pixel music notes.
- [ ] **T-009: Pomodoro / Focus Companion Mode**
  - Built-in tray timer: pets nap/read quietly during 25m focus blocks and hop with a break reminder at 0:00.
- [ ] **T-010: Community Pet Registry CLI (`diypets install <pet>`) & Equipable Hats**
  - Curated GitHub registry index for 1-command community pet installs + decoupled runtime eye-tracking/blinking and equipable hats (party hat, headphones, scarf).