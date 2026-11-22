# Contributing to DIYPets

Thanks for helping make DIYPets better!

## Dev Setup
```bash
git clone https://github.com/smresponsibilities/diypets.git
cd diypets
cargo test
```

Run individual binaries during development:
```bash
cargo run --bin diypets
cargo run --bin diypets-settings
cargo run --bin diypets-creator
```

## Code Guidelines
- Keep the single-crate layout clean and cross-platform (windows crate must stay gated under cfg(windows)).
- Never block the winit event loop or add heavy background allocations per frame.
- Run cargo test and cargo clippy --all-targets before submitting a PR.
- Keep cargo package --allow-dirty under crates.io's **10 MB** limit (new large community packs belong in the lazy-loaded catalog, not inside the core crate bundle).

## Submitting a Pet Pack
1. Place your pack in ssets/<pack_id>/deskpet.xml (use lowercase snake_case for <pack_id>).
2. Ensure the character natively **faces LEFT** on the sprite sheet.
3. Verify zero foot baseline jitter across walking frames and clean transparent edges (no halo/fringe pixels).
4. Provide core animations: Spawn, Walk (with non-zero X movement), Idle/Sit, Fall, Drag, Sleep, and Climb (where applicable).
5. Fill in <Header><Title> and <Author> so proper credit shows in Settings.
6. Test your pack in both diypets and diypets-settings before opening your PR.