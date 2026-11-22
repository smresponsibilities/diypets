# DIYPets Complete Ticket Backlog (`smresponsibilities/diypets`)

Legend: **Priority** (`P0` Critical -> `P3` Nice-to-have) · **Effort** (`S` <2h, `M` half-day, `L` 1-2d) · `[good-first-issue]` suitable for new contributors.

---

## Done (v0.1.0 -> v0.1.2)
- [x] **T-001** Build stability (`image` PNG-only, `jobs = 1`, `line-tables-only`, HTTP/1.1 publish config).
- [x] **T-002** Rebrand `deskpet` -> `diypets` (`diypets`, `diypets-settings`, `diypets-creator`), update yanked `clipboard-win`.
- [x] **T-003** Published `diypets` on crates.io & slimmed `.crate` from **7.18 MB -> 1.09 MB**.
- [x] **T-004** Open-source kit (`README.md`, `CONTRIBUTING.md`, `CREDITS.md`, `CHANGELOG.md`, `AGENT_PET_GUIDE.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, Issue/PR templates, CI + Release workflows).
- [x] **T-005** Lazy eSheep catalog (`catalog.json`, `catalog_icons/`, `ureq` on-demand download, right-click delete, Import `.xml`).
- [x] **T-006** Hidden Windows console in release builds, crisp 64x64 startup icons, native cross-platform **Start on Login**.
- [x] **T-008 / T-009 / T-010** Fast 2-row Settings (**Made by SM** with all 5 `By SM` + **eSheep** with `Adrianotiger/desktopPet` credit), 128px previews, auto-refresh.
- [x] **T-012** DIYPets Studio V2 + Agent CLI (5 archetypes, unlimited parts, 5 faces, Custom Anims, 2D SVG auto-splitter, Seamless Silhouette Outlining, `--from-svg`/`--build`/`--schema`).
- [x] **T-018** Multi-Pet Herd Mode (up to 8 simultaneous pets, multi-HWND exclusion, Settings Active Herd bar & `+ Add`, tray Add/Remove/Sleep All).
- [x] **T-020 / T-021** Stripped 155 MiB Git bloat (`9.49 MiB` push), live `main` branch + `v0.1.1`/`v0.1.2` CI & Releases.
- [x] **T-024** Automated Stress Test Suite (`tests/stress_test.rs`: 40,000-tick 8-pet physics, roster sim, concurrent atomic settings, SVG fuzz) + stranded monitor-shrink clamp fix.
- [x] **T-025** Highland Cow Brand Kit (`brand/social-preview.png` 1280x640, `brand/logo-512.png`, `brand/diypets.ico`) + full `ROADMAP.md`.

---

## Pillar 1: Desktop Runtime, Physics & Performance
- [x] **T-014A** `[P0][M]` **Fix True Per-Pixel Alpha Click-Through:** On Windows `UpdateLayeredWindow` passes clicks through `alpha == 0` pixels natively when `WS_EX_TRANSPARENT` is NOT set; stop calling `set_cursor_hittest(false)` globally so users can always drag opaque pet pixels while clicking through transparent corners!
- [x] **T-014B** `[P0][M]` **Direct Right-Click Context Menu on Pets:** Right-clicking any pet opens a quick menu (Pet/Headpat, Sleep, Sit, Clone, Dismiss This Pet, Open Settings).
- [ ] **T-026** `[P1][S]` **Embed Highland Cow `.ico` into Windows `.exe` Binaries:** Add `build.rs` with `winresource` (Windows-only build-dep) using `brand/diypets.ico` so Explorer & Task Manager show the Highland Cow logo.
- [x] **T-027** `[P1][M]` **Scaled Frame Cache & Smart Hi-Res Filtering:** Cache scaled ARGB frames by `(frame_idx, moving_left, pet_size)` (90% less CPU while walking) and use Lanczos/bilinear scaling for hi-res >=96px art vs. nearest-neighbor for retro pixel art.
- [x] **T-028** `[P1][M]` **Multi-Monitor Left Wall & Negative Coordinates:** Pass `min_x` / `wall_left` into `DeskpetState::tick` so pets roam smoothly onto monitors positioned to the left/above the primary screen.
- [ ] **T-029** `[P1][S]` `[good-first-issue]` **Replace `meval` With Zero-Dep Math Evaluator:** Eliminates the `nom v1.2.4` Rust future-incompatibility compiler warning and trims dependency count.
- [ ] **T-013** `[P2][M]` **Retire Duplicate Format v0 Runtime (`state.rs`, `pack.rs`, `sprite.rs`) & Scratch Bins:** Keep one unified `deskpet_state.rs` engine and move dev tools (`check_png`, `gen_sprites`, `gen_art`) out of `src/bin/`.

---

## Pillar 2: Settings UI & UX Polish
- [ ] **T-030** `[P1][M]` **Modern Tabbed Settings UI:** Split into `Gallery` · `Active Herd` · `Behavior & System` · `About & Credits` with spacious elevated cards and cleaner spacing.
- [ ] **T-031** `[P1][S]` **Animated Card Hover Previews:** Animate a gentle 2-frame walk/bounce when hovering over a pet card in Settings.
- [ ] **T-032** `[P1][M]` **Per-Pet Herd Size & Speed Overrides:** Let each pet in the Active Herd have its own size/speed (e.g., 160px Highland Cow + 64px eSheep).
- [ ] **T-033** `[P1][S]` **Drag-and-Drop `.xml` / `.svg` Install & "Launch Pet" Button:** Drop files directly onto Settings to install/activate, plus a 1-click button to start `diypets` if it isn't currently running.
- [ ] **T-034** `[P2][S]` **Live Remote `catalog.json` Sync:** Fetch latest `catalog.json` in background from GitHub (with instant embedded fallback offline) so new community packs appear without needing a binary update.
- [ ] **T-035** `[P2][S]` `[good-first-issue]` **First-Run Welcome Tip:** Subtle one-time toast/banner showing how to drag pets, open Settings from the tray, and launch Studio V2.

---

## Pillar 3: DIYPets Studio V2 -> V3
- [x] **T-036** `[P0][M]` **Pro Canvas Ergonomics:** Mouse-wheel zoom, Spacebar/middle-click pan, Light/Dark checkerboard toggle (fixes drawing dark ink on dark checks!), marquee Select/Move/Copy/Paste/Scale, opacity slider, and recent colors bar.
- [ ] **T-037** `[P0][L]` **Sprite Sheet & Animated GIF Importer:** Import any PNG strip/grid or `.gif`, choose `TilesX x TilesY`, map frames to Walk/Idle/Sleep/Fall, auto-compute `<GroundY>`, and export a playable pet in 10 seconds.
- [x] **T-038** `[P1][M]` **Timeline Scrubber & Pause-on-Keyframe Preview:** Let users scrub/pause exact keyframes in `3. Custom Anims`, duplicate/reorder frames, and preview individual poses live.
- [ ] **T-039** `[P1][M]` **Open & Edit Existing `.xml` Packs in Studio:** Load any `deskpet.xml` back into Studio to tweak `<GroundY>`, speed, metadata, or animations.
- [x] **T-040** `[P1][M]` **Part Z-Order Reordering & 2-Bone Knee/Elbow IK:** Add Up/Down layer ordering buttons for parts and optional 2-bone limb bend for natural walks.
- [ ] **T-041** `[P2][M]` **Upgraded 2D SVG Importer + Import Preview Modal:** Add `<linearGradient>`, CSS `<style>` rules, rounded `<rect rx/ry>`, elliptical arcs (`A`), and an interactive role-reassignment preview before importing.
- [ ] **T-042** `[P2][S]` `[good-first-issue]` **Add Dragon (4-Leg + Wings + Tail) & Fox Templates:** Showcase multi-part rigging right out of the box.

---

## Pillar 4: Bundled Roster Animation Parity (`highland_cow` Gold Standard)
- [x] **T-011A** `[P0][M]` **Fix `stegosaurus`:** Fix 13px floating `GroundY` (`110` -> `98`) and add `sleep` (`id="13"`) & `climb` (`id="19"`).
- [x] **T-011B** `[P0][M]` **Fix `goat`:** Fix 7px vertical foot drift on `WALK_1` frame `2` (`bot=111..118`) and add `climb` (`id="19"`).
- [x] **T-011C** `[P1][S]` **Polish `dog` & `polar_bear`:** Tighten `dog` `GroundY` (`103` -> `101`) & recenter `polar_bear` non-walk frames (`cx_spread=13px`).
- [ ] **T-011D** `[P1][S]` `[good-first-issue]` **Fix `negima` eSheep Pack:** Add missing `walk_x` travel and verify clean turnback on packs missing `climb`.

---

## Pillar 5: Viral Delight & Developer Companion Features
- [x] **T-019A** `[P0][L]` **AI Agent MCP / Local HTTP Server + Speech Bubbles:** Optional `127.0.0.1:3721/mcp` & `/react` endpoint + `diypets react --bubble "Tests Passed!" --mood happy` so Claude Code, Cursor, Codex, and CI builds trigger live pet reactions & tiny speech bubbles!
- [x] **T-019B** `[P1][S]` **Ship Agent Skill (`skills/diypets-creator/SKILL.md`):** Drop-in skill file so coding agents automatically know how to create semantic SVGs and run `diypets-creator --from-svg --activate`.
- [x] **T-043** `[P1][M]` **Headpats & Floating Pixel Hearts:** Rubbing cursor back-and-forth over a pet spawns cute floating hearts and triggers the `Happy` face!
- [ ] **T-044** `[P1][M]` **Play Mode (Cursor Chase & Tossable Toy/Treat):** Toggle Neko-style mouse chase or drop a bouncy pixel ball/treat that your herd chases across the taskbar.
- [ ] **T-023** `[P2][M]` **Pomodoro / Break Buddy, Typing Buddy & Optional Cozy SFX:** Opt-in stretch reminder bubble, subtle BongoCat-style keystroke bounce, and muted-by-default sound effects.

---

## Pillar 6 & 7: Cross-Platform, Packaging & GitHub Growth
- [ ] **T-022** `[P0][S]` **Upload GitHub Social Preview (`brand/social-preview.png`), Set 20 GitHub Topics & Record 10s Hero Demo GIF.**
- [ ] **T-016** `[P1][M]` **Wayland/GNOME Graceful Detection & macOS/Linux QA:** Detect Wayland, gracefully disable unsupported window-ledge climbing with a tooltip, and verify Retina HiDPI on macOS.
- [ ] **T-017** `[P1][M]` **Package Managers:** Publish manifests for **Winget**, **Scoop**, **Homebrew Tap**, and **Linux AppImage / AUR**.
- [ ] **T-045** `[P2][S]` **Community Pack Auto-Catalog GitHub Action & Update Notifier:** Automatically rebuild `catalog.json` + icons when a community pet pack PR is merged, and show a subtle "Update Available" badge in Settings.