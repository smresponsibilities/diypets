---
name: diypets-creator
description: Create, rig, animate, and react with DIYPets desktop mascots via 2D SVG CLI and local MCP/HTTP API.
---

# DIYPets Agent Skill

## 1. Build a Desktop Pet From a 2D SVG (One Command)
Create a 128x128 SVG facing **LEFT** with semantic <g id="..."> groups (body, front_leg, back_leg, wing, tail, ear, face, face_blink, face_happy, face_sleep, face_surprised), then run:

```bash
diypets-creator --from-svg pet.svg --name "My Pet" --archetype quadruped --author "AI Agent" --activate
```
Archetypes: quadruped | biped | bird | crawler (snake wave) | swimmer (fish wave).

## 2. Trigger Live Desktop Pet Reactions (CLI / HTTP / MCP)
While diypets is running, it listens on http://127.0.0.1:3721:
- **CLI:** diypets react --mood happy --text "Tests passed!" (happy | hop | sleep | walk)
- **HTTP:** POST http://127.0.0.1:3721/react with {"mood":"happy","text":"Done!"}
- **MCP JSON-RPC:** POST http://127.0.0.1:3721/mcp (tools/list, tools/call -> react_pet, spawn_pet)