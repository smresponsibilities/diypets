# Building DIYPets With AI Agents

DIYPets is designed so AI coding agents (Claude Code, Cursor, Codex, Gemini CLI) can design, rig, animate, and install complete desktop pets automatically.

## Workflow 1: Generate a 2D SVG -> Auto-Rigged Desktop Pet

Write a standard 128x128 SVG with the mascot **facing LEFT** and group parts using semantic <g id="..."> tags:

| SVG <g id="..."> | Auto-Mapped Role | Animation Behavior |
| :--- | :--- | :--- |
| ody, 	orso, head | Core Body | Grounded breathing, tilt, or wave slither (Crawler/Swimmer) |
| ront_leg, rm | Front Limb (paired) | Alternating gait swing + shaded far-side twin |
| ack_leg, leg, oot | Back Limb (paired) | Opposite-phase gait swing + shaded far-side twin |
| wing, side_fin | Wing / Fin (paired) | Flapping / paddling motion |
| 	ail | Tail (behind body) | Continuous wag / swim fin flutter |
| ear, horn, crest, ntenna | Ear / Head Accent | Subtle secondary bounce |
| hat, prop, extra | Custom Accessory | Attached to torso |
| ace / ace_blink / ace_happy / ace_sleep / ace_surprised | Expression Overlays | Automatically swapped during Walk, Idle, Sit, Sleep, Drag & Fall |

### Supported Archetypes
quadruped · iped · ird · crawler (snake wave) · swimmer (fish wave)

### Example Agent SVG
```xml
<svg viewBox="0 0 128 128" xmlns="http://www.w3.org/2000/svg">
  <g id="body">
    <ellipse cx="64" cy="68" rx="24" ry="16" fill="#58b8f5" stroke="#1e2a3a" stroke-width="2"/>
  </g>
  <g id="face">
    <circle cx="46" cy="62" r="3" fill="#1e2a3a"/>
  </g>
  <g id="wing">
    <ellipse cx="68" cy="66" rx="14" ry="8" fill="#3a8ad6"/>
  </g>
  <g id="tail">
    <polygon points="86,66 108,58 106,74" fill="#3a8ad6"/>
  </g>
</svg>
```

### Compile & Install via CLI (Coming in Next Commit / Studio V2 CLI)
```bash
diypets-creator --from-svg bird.svg --name "Sky Bird" --archetype bird --author "AI Agent" --activate
diypets-creator --build my_pet.diypet.json --activate
diypets-creator --schema-out diypet_schema.json
```