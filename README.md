# diypets

Cross-platform desktop mascot engine written in Rust (~7.8 MB idle RAM), with multi-pet herds, a built-in 2D/SVG sprite studio, and an MCP server for coding agents.

Live sandbox: [shivammahajan.com/diypets](https://shivammahajan.com/diypets/) · Package: [crates.io/crates/diypets](https://crates.io/crates/diypets)

![DIYPets built-in mascots](docs/og-image.png)

## Install

Windows (PowerShell):
```powershell
irm [https://raw.githubusercontent.com/smresponsibilities/diypets/main/install.ps1](https://raw.githubusercontent.com/smresponsibilities/diypets/main/install.ps1) | iex
```

macOS / Linux:
```bash
curl -fsSL [https://raw.githubusercontent.com/smresponsibilities/diypets/main/install.sh](https://raw.githubusercontent.com/smresponsibilities/diypets/main/install.sh) | bash
```

From source via Cargo:
```bash
cargo install diypets
```

## Usage

```bash
diypets                       # start desktop pet runner
diypets say "Build passed"    # trigger a speech bubble on the active pet
diypets mcp                   # run stdio MCP server for Claude / Cursor / Codex
diypets hooks install all     # auto-wire Claude Code & Cursor lifecycle hooks
diypets export-gif pet.xml    # export a lossless transparent looping GIF
diypets doctor                # print config path, logs, herd state, and build info
diypets --version             # print version, git commit, and target triple
diypets-settings              # open herd & behavior settings UI
diypets-creator               # open 2D/SVG pet studio
```

Right-click any pet or the system tray icon to drop a bouncy ball, toggle Follow Cursor, headpat, sleep, or clone up to 8 pets. Settings live in `%APPDATA%\diypets\settings.toml` (Windows) or `~/.config/diypets/settings.toml` (macOS/Linux); rotating logs auto-prune after 7 days in the sibling `logs/` directory.

## What's included

| Component | Details |
| :--- | :--- |
| Runner (`diypets`) | Per-pixel layered transparency (Win32 `UpdateLayeredWindow`, Cocoa, X11/Wayland), window-ledge walking, squash-and-stretch physics, fullscreen auto-hide, multi-pet herds (up to 8). |
| Packs | 5 built-in pixel mascots (Highland Cow, Dog, Polar Bear, Goat, Stegosaurus) plus on-demand eSheep XML pack catalog (27+ community packs). |
| Studio (`diypets-creator`) | 128x128 layered pixel editor, Marquee Select, Mirror X, Onion Skinning, 1-click Auto-Outline, SVG auto-rigging, 4x EPX RotSprite rotation, live sandbox, and XML export. |
| Agent MCP (`diypets mcp`) | Stdio & local HTTP bridge exposing `pet_say`, `pet_react`, `pet_drop_ball`, `pet_spawn`, `pet_set_follow_cursor`, and `pet_status`. |

## MCP configuration

Add to Claude Desktop, Cursor, or Claude Code (`mcp.json`):

```json
{
  "mcpServers": {
    "diypets": {
      "command": "diypets",
      "args": ["mcp"]
    }
  }
}
```

## Building & testing

```bash
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
python scripts/validate_sprites.py   # checks sprite transparency, foot baselines, and generates walk GIFs
```

See [CONTRIBUTING.md](CONTRIBUTING.md) and [TICKETS.md](TICKETS.md) for open starter issues and roadmap items.

## License

Dual-licensed under MIT or Apache-2.0.
