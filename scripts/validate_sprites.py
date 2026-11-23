#!/usr/bin/env python3
"""Automated Sprite & GIF Visual Validation Pipeline for DIYPets."""
import base64, io, os, re, sys
from PIL import Image, ImageDraw

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
OUT_DIR = os.path.join(ROOT, "target", "visual-audit")
os.makedirs(OUT_DIR, exist_ok=True)

PETS = ["highland_cow", "dog", "polar_bear", "goat", "stegosaurus"]
errors = []
strip_rows = []

for pid in PETS:
    xp = os.path.join(ROOT, "assets", pid, "deskpet.xml")
    if not os.path.isfile(xp):
        errors.append(f"[{pid}] Missing {xp}")
        continue
    txt = open(xp, "r", encoding="utf-8").read()
    m = re.search(r"<Png[^>]*>([\s\S]*?)</Png>", txt, re.I)
    tx = int(re.search(r"<TilesX>(\d+)</TilesX>", txt, re.I).group(1))
    ty = int(re.search(r"<TilesY>(\d+)</TilesY>", txt, re.I).group(1))
    sheet = Image.open(io.BytesIO(base64.b64decode(m.group(1).strip()))).convert("RGBA")
    tw, th = sheet.width // tx, sheet.height // ty

    t0 = sheet.crop((0, 0, tw, th))
    for cx, cy in ((0, 0), (tw - 1, 0), (0, th - 1), (tw - 1, th - 1)):
        if t0.getpixel((cx, cy))[3] != 0:
            errors.append(f"[{pid}] Tile 0 corner ({cx},{cy}) is not transparent")

    m_idle = re.search(r'<Animation[^>]*id="2"[^>]*>([\s\S]*?)</Animation>', txt, re.I)
    idle_ids = [int(x) for x in re.findall(r"<Frame>(\d+)</Frame>", m_idle.group(1))] if m_idle else [0]
    m_walk = re.search(r'<Animation[^>]*id="1"[^>]*>([\s\S]*?)</Animation>', txt, re.I)
    walk_ids = [int(x) for x in re.findall(r"<Frame>(\d+)</Frame>", m_walk.group(1))][:4] if m_walk else [0, 1, 2, 3]
    while len(walk_ids) < 4:
        walk_ids.append(walk_ids[-1])

    seq = [idle_ids[0]] + walk_ids
    bottoms = []
    zoom_frames = []
    for fid in seq:
        tile = sheet.crop(((fid % tx) * tw, (fid // tx) * th, (fid % tx + 1) * tw, (fid // tx + 1) * th))
        bb = tile.getbbox()
        if bb:
            bottoms.append(bb[3])
        zoom_frames.append(tile.resize((128, 128), Image.NEAREST))

    walk_bottoms = bottoms[1:] if len(bottoms) > 1 else bottoms
    if walk_bottoms and (max(walk_bottoms) - min(walk_bottoms)) > 4:
        errors.append(f"[{pid}] Foot baseline jitter too high across walk: {walk_bottoms}")

    gif_frames = []
    for zf in zoom_frames[1:]:
        bg = Image.new("RGBA", (144, 144), (23, 44, 50, 255))
        bg.alpha_composite(zf, (8, 8))
        gif_frames.append(bg.convert("P", palette=Image.ADAPTIVE))
    gif_frames[0].save(
        os.path.join(OUT_DIR, f"{pid}_walk.gif"),
        save_all=True, append_images=gif_frames[1:], duration=135, loop=0,
    )
    strip_rows.append((pid, zoom_frames))

if strip_rows:
    cw, ch = 5 * 136 + 150, len(strip_rows) * 144 + 24
    cs = Image.new("RGBA", (cw, ch), (14, 18, 24, 255))
    draw = ImageDraw.Draw(cs)
    for r_idx, (pid, zframes) in enumerate(strip_rows):
        y = 16 + r_idx * 144
        draw.text((14, y + 54), pid, fill=(230, 237, 243, 255))
        for c_idx, zf in enumerate(zframes):
            x = 145 + c_idx * 136
            draw.rectangle((x, y, x + 128, y + 128), fill=(23, 44, 50, 255), outline=(64, 78, 92, 255))
            cs.alpha_composite(zf, (x, y))
    cs.save(os.path.join(OUT_DIR, "contact_sheet.png"), "PNG")
    print(f"Generated contact_sheet.png & walk GIFs in {OUT_DIR}")

if errors:
    for e in errors:
        print("VALIDATION ERROR:", e, file=sys.stderr)
    sys.exit(1)
print("All sprite visual invariants passed!")
