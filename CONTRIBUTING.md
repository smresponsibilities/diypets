# Contributing to DIYPets

Thanks for helping make DIYPets faster, cuter, and easier to customize!

## Local Setup

1. Install stable Rust (`rustup toolchain install stable`) and Python 3 + Pillow (`pip install pillow`) if editing sprite sheets.
2. On Windows, if `C:` disk space is tight during Cargo builds, route build artifacts to a secondary drive:
   ```powershell
   $env:CARGO_TARGET_DIR = "D:\cargo-tmp\target"
   ```
3. Verify tests pass locally:
   ```bash
   cargo test --all-targets
   ```

## PR Validation Checklist

1. **Unit & Regression Tests:** `cargo test --all-targets` passes with `0` failures.
2. **Lint & Formatting:** `cargo clippy --all-targets -- -D warnings` passes cleanly.
3. **Sprite & GIF Visual Validation:** If your PR touches `assets/` or `docs/sprites/`, run:
   ```bash
   python scripts/validate_sprites.py
   ```
   Open `target/visual-audit/contact_sheet.png` and `target/visual-audit/*_walk.gif` to confirm transparent corners, locked foot baselines, and crisp eyes.

## Where to Start

- **Good First Issues:** Tooltips, new unit tests, eSheep pack metadata, docs improvements.
- **Medium Features:** Studio 8-bit brush tools, GIF exporter, Scratch-style Costumes flipbook, Coding-Agent hook presets.
- Browse [`TICKETS.md`](TICKETS.md) and open an issue using our templates before starting large features.
