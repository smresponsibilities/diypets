---
name: diypets-creator
description: Create, rig, animate, and control DIYPets desktop mascots via 2D SVG CLI and local MCP/HTTP API (6 tools).
---

# DIYPets Agent Skill

## 1. Build a Desktop Pet From a 2D SVG (One Command)
Create a 128x128 SVG facing **LEFT** with semantic <g id="..."> groups (body, front_leg, back_leg, wing, tail, ear, face, face_blink, face_happy, face_sleep, face_surprised), then run:

```bash
diypets-creator --from-svg pet.svg --name "My Pet" --archetype quadruped --author "AI Agent" --activate
```
Archetypes: quadruped | biped | bird | crawler | swimmer.

## 2. Control Live Desktop Pets (CLI / HTTP / MCP on http://127.0.0.1:3721)
- **CLI shortcuts:**
  - diypets say "Tests passed!"
  - diypets ball
  - diypets react --mood happy --text "PR #42 merged!"
- **HTTP endpoints:** GET /status · GET /herd · POST /react · POST /mcp
- **MCP JSON-RPC 2.0 tools (/mcp):**
  1. react_pet (mood: happy|hop|sleep|walk|ball, text?, duration_ms?)
  2. say (text, duration_ms?)
  3. drop_ball ()
  4. spawn_pet (pack_id?)
  5. set_follow_cursor (enabled: bool)
  6. list_herd ()