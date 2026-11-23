---
name: diypets-creator
description: Create, auto-rig, animate, and control ~7.8 MB native Rust DIYPets desktop mascots directly from AI prompts using the create_pet_from_svg MCP tool, 2D SVG CLI, and 1-click Claude Code / Cursor hooks.
---

# DIYPets Creator & Agent Companion Skill

Give Claude Code, Cursor, Codex, or Claude Desktop the ability to generate custom rigged pixel-art desktop pets in 1 prompt and react to coding lifecycle events (~7.8 MB idle RAM, cross-platform Rust).

## 1. Connect Native stdio MCP Server

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

Install 1-click lifecycle hooks:
```bash
diypets hooks install all
```

## 2. Create & Spawn Any Custom Desktop Pet in 1 Tool Call

1. Author a clean 128x128 SVG facing LEFT (`viewBox="0 0 128 128"`).
2. Group semantic parts with `<g id="...">`: `body`, `front_leg`, `back_leg`, `wing`, `tail`, `ear`, `face`, `face_blink`, `face_happy`, `face_sleep`, `face_surprised`.
3. Call `create_pet_from_svg` with `{ "svg": "...", "name": "...", "archetype": "quadruped|biped|bird|crawler|swimmer", "spawn": true }`.

CLI fallback:
```bash
diypets-creator --from-svg pet.svg --name "My Pet" --archetype quadruped --author "AI Agent" --activate
```

## 3. All 7 MCP Tools (`diypets mcp`)

- `create_pet_from_svg`: Generate, auto-rig, install, and spawn a custom desktop pet from inline SVG or `.svg` file.
- `react_pet`: Trigger `happy`, `hop`, `sleep`, `walk`, or `ball` with an optional speech bubble.
- `say`: Show a calm status speech bubble without forcing a jump.
- `drop_ball`: Drop a bouncy pixel toy ball for the active herd to chase.
- `spawn_pet`: Add an existing mascot to the herd (up to 8).
- `set_follow_cursor`: Toggle Play Mode cursor following.
- `list_herd`: Read-only list of currently active desktop pets.
