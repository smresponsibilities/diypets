---
name: diypets-creator
description: Create, rig, animate, and control DIYPets desktop mascots via 2D SVG CLI and stdio/HTTP MCP API (6 tools).
---

# DIYPets Agent Skill

## 1. Connect via Native stdio MCP (Claude Desktop / Cursor / Claude Code)
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

## 2. Build a Desktop Pet From a 2D SVG (One Command)
Create a 128x128 SVG facing **LEFT** with semantic <g id="..."> groups (body, front_leg, back_leg, wing, tail, ear, face, face_blink, face_happy, face_sleep, face_surprised), then run:

```bash
diypets-creator --from-svg pet.svg --name "My Pet" --archetype quadruped --author "AI Agent" --activate
```
Archetypes: quadruped | biped | bird | crawler | swimmer.

## 3. Control Live Desktop Pets (CLI / HTTP / MCP)
- **CLI:** diypets say "Tests passed!" · diypets ball · diypets react --mood happy --text "PR merged!"
- **HTTP (127.0.0.1:3721):** GET /status · GET /herd · POST /react · POST /mcp
- **MCP tools:** react_pet, say, drop_ball, spawn_pet, set_follow_cursor, list_herd