import base64
import io
import json
import os
import subprocess
import sys
import xml.etree.ElementTree as ET
from PIL import Image, ImageDraw

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT_DIR = os.path.join(ROOT, "target", "visual-audit", "autorig_benchmark")
os.makedirs(OUT_DIR, exist_ok=True)
PETS = ["highland_cow", "dog", "polar_bear", "goat", "stegosaurus"]


def load_pack_frames(pet_id):
    xml_path = os.path.join(ROOT, "assets", pet_id, "deskpet.xml")
    tree = ET.parse(xml_path)
    root = tree.getroot()
    img_node = root.find("./Image")
    tx = int((img_node.findtext("TilesX") or "1").strip())
    ty = int((img_node.findtext("TilesY") or "1").strip())
    png_b64 = "".join((img_node.findtext("Png") or "").split())
    sheet = Image.open(io.BytesIO(base64.b64decode(png_b64))).convert("RGBA")
    tw, th = sheet.width // tx, sheet.height // ty

    walk_ids = []
    for anim in root.findall("./Animation"):
        if anim.attrib.get("id") == "1":
            for fr in anim.findall("./Sequence/Frame"):
                val = int((fr.text or "0").strip())
                if val not in walk_ids:
                    walk_ids.append(val)
            break
    if len(walk_ids) < 4:
        walk_ids = [0, 1, 2, 3]

    def tile(idx):
        cx = idx % tx
        cy = idx // tx
        return sheet.crop((cx * tw, cy * th, (cx + 1) * tw, (cy + 1) * th))

    return tile(0), [tile(i) for i in walk_ids[:4]], (tw, th)


def make_checker(w, h, sq=8):
    bg = Image.new("RGBA", (w, h), (22, 26, 36, 255))
    px = bg.load()
    for y in range(h):
        for x in range(w):
            if ((x // sq) + (y // sq)) % 2 == 0:
                px[x, y] = (32, 38, 52, 255)
    return bg


def foot_y(im):
    w, h = im.size
    px = im.load()
    for y in range(h - 1, -1, -1):
        for x in range(w):
            if px[x, y][3] > 24:
                return y
    return h - 1


def main():
    print("=== EMPIRICAL DRY-RUN BENCHMARK: AUTO-RIG vs 5 REAL AUTHORED MASCOT SHEETS ===")
    failures = []
    rows_imgs = []

    for i in range(len(PETS)):
        pet = PETS[i]
        idle_im, real_walk, (tw, th) = load_pack_frames(pet)
        tmp_in = os.path.join(OUT_DIR, pet + "_idle.bin")
        tmp_out = os.path.join(OUT_DIR, pet + "_autorig.json")
        with open(tmp_in, "wb") as f:
            f.write(idle_im.tobytes())

        node_code = (
            'const fs = require("fs");\n'
            'const { buildAutoRig, renderRiggedFrame, encodeGif89a } = require("./docs/gif_maker.js");\n'
            f'const raw = new Uint8ClampedArray(fs.readFileSync({json.dumps(tmp_in)}));\n'
            f'const rig = buildAutoRig(raw, {tw}, {th}, true, "auto");\n'
            'const frames = [];\n'
            'for (let k = 0; k < 8; k++) frames.push(renderRiggedFrame(rig, "walk", k, 8));\n'
            'const dbg = rig.grid;\n'
            'let legDiff = 0, upperDiff = 0;\n'
            'for (let y = 0; y < rig.h; y++) for (let x = 0; x < rig.w; x++) {\n'
            '  const p = (y * rig.w + x) * 4;\n'
            '  if (frames[0][p+3] !== frames[2][p+3]) { if (y >= rig.hipY) legDiff++; else upperDiff++; }\n'
            '}\n'
            'const gif = encodeGif89a(frames, rig.w, rig.h, 12, true);\n'
            f'fs.writeFileSync({json.dumps(os.path.join(OUT_DIR, pet + "_autorig_walk.gif"))}, Buffer.from(gif));\n'
            'const out = {\n'
            '  bodyType: rig.bodyType, w: rig.w, h: rig.h, hipY: rig.hipY, facesLeft: rig.facesLeft,\n'
            '  maxRuns: rig.maxRuns, legDiff, upperDiff,\n'
            '  framesB64: [0,2,4,6].map(k => Buffer.from(frames[k]).toString("base64")),\n'
            '  dbgB64: Buffer.from(dbg).toString("base64")\n'
            '};\n'
            f'fs.writeFileSync({json.dumps(tmp_out)}, JSON.stringify(out));\n'
        )
        res = subprocess.run(["node", "-e", node_code], cwd=ROOT, text=True)
        if res.returncode != 0:
            failures.append(pet + ": node error")
            continue

        info = json.load(open(tmp_out, "r", encoding="utf-8"))
        os.remove(tmp_in)
        os.remove(tmp_out)

        auto_frames = []
        for k in range(len(info["framesB64"])):
            raw_f = base64.b64decode(info["framesB64"][k])
            auto_frames.append(Image.frombytes("RGBA", (info["w"], info["h"]), raw_f))
        dbg_im = Image.frombytes("RGBA", (info["w"], info["h"]), base64.b64decode(info["dbgB64"]))

        # Also export horizontal 8x1 PNG sprite sheet for direct pixel-by-pixel inspection
        sheet8 = Image.new("RGBA", (info["w"] * 4, info["h"]), (0, 0, 0, 0))
        for k in range(4):
            sheet8.paste(auto_frames[k], (k * info["w"], 0))
        sheet8.save(os.path.join(OUT_DIR, pet + "_autorig_sheet.png"))

        # Verify visual invariants (sprite-visual-validation)
        auto_foots = [foot_y(im) for im in auto_frames]
        foot_jitter = max(auto_foots) - min(auto_foots)
        real_foots = [foot_y(im) for im in real_walk]
        real_jitter = max(real_foots) - min(real_foots)

        # Verify Pillow decodes the exported GIF cleanly
        gif_path = os.path.join(OUT_DIR, pet + "_autorig_walk.gif")
        gim = Image.open(gif_path)
        assert gim.format == "GIF" and getattr(gim, "n_frames", 1) == 8
        gim.seek(0)
        corner_alpha = gim.convert("RGBA").getpixel((0, 0))[3]
        gim.close()

        ok = (
            info["bodyType"] == "quadruped"
            and info["legDiff"] >= 16
            and foot_jitter <= 2
            and corner_alpha == 0
        )
        status = "PASS" if ok else "FAIL"
        print(
            f"  [{status}] {pet:13s} | type={info['bodyType']:9s} | grid={info['w']}x{info['h']} "
            f"| runs={info['maxRuns']} | leg_motion={info['legDiff']:3d}px | upper={info['upperDiff']:3d}px "
            f"| foot_jitter={foot_jitter}px (real={real_jitter}px)"
        )
        if not ok:
            failures.append(f"{pet}: type={info['bodyType']} legDiff={info['legDiff']} jitter={foot_jitter}")

        # Build side-by-side animated GIF: REAL (left) vs AUTO-RIG (right) at 128x128 each
        side_frames = []
        for k in range(4):
            pair = make_checker(256, 128)
            r_im = real_walk[k].resize((128, 128), Image.NEAREST)
            a_sc = max(1, min(124 // info["w"], 124 // info["h"]))
            a_im = auto_frames[k].resize((info["w"] * a_sc, info["h"] * a_sc), Image.NEAREST)
            pair.alpha_composite(r_im, (0, 0))
            pair.alpha_composite(a_im, (128 + (128 - a_im.width) // 2, 124 - a_im.height))
            d = ImageDraw.Draw(pair)
            d.text((6, 4), pet + " REAL", fill=(126, 232, 250, 255))
            d.text((134, 4), "AUTO-RIG (" + info["bodyType"] + ")", fill=(130, 250, 150, 255))
            side_frames.append(pair.convert("P", palette=Image.ADAPTIVE))
        side_frames[0].save(
            os.path.join(OUT_DIR, pet + "_real_vs_autorig.gif"),
            save_all=True,
            append_images=side_frames[1:],
            duration=140,
            loop=0,
        )

        # Build comparison strip row: [DBG RIG | AUTO 0..3 | REAL 0..3]
        row = make_checker(128 * 9, 128)
        d_sc = max(1, min(124 // info["w"], 124 // info["h"]))
        d_im = dbg_im.resize((info["w"] * d_sc, info["h"] * d_sc), Image.NEAREST)
        row.alpha_composite(d_im, ((128 - d_im.width) // 2, 124 - d_im.height))
        for k in range(4):
            a_im = auto_frames[k].resize((info["w"] * d_sc, info["h"] * d_sc), Image.NEAREST)
            row.alpha_composite(a_im, (128 * (k + 1) + (128 - a_im.width) // 2, 124 - a_im.height))
            r_im = real_walk[k].resize((128, 128), Image.NEAREST)
            row.alpha_composite(r_im, (128 * (k + 5), 0))
        rows_imgs.append(row)

    # Save combined 5-pet comparison contact sheet
    sheet_out = Image.new("RGBA", (128 * 9, 128 * len(rows_imgs)), (20, 24, 34, 255))
    for i in range(len(rows_imgs)):
        sheet_out.paste(rows_imgs[i], (0, i * 128))
    cs_path = os.path.join(OUT_DIR, "all_pets_comparison_sheet.png")
    sheet_out.save(cs_path)
    print("Saved 5-pet visual comparison sheet:", cs_path)

    # Also test Sprite Sheet Slicer on real 9-column docs/sprites/polar_bear.png!
    pb_strip = os.path.join(ROOT, "docs", "sprites", "polar_bear.png")
    if os.path.isfile(pb_strip):
        sp = Image.open(pb_strip).convert("RGBA")
        sp_bin = os.path.join(OUT_DIR, "pb_strip.bin")
        with open(sp_bin, "wb") as f:
            f.write(sp.tobytes())
        slice_test = (
            'const fs = require("fs");\n'
            'const { detectSpriteSheetGrid, sliceSpriteSheet, encodeGif89a } = require("./docs/gif_maker.js");\n'
            f'const raw = new Uint8ClampedArray(fs.readFileSync({json.dumps(sp_bin)}));\n'
            f'const g = detectSpriteSheetGrid(raw, {sp.width}, {sp.height});\n'
            'if (!g.isSheet || g.cols !== 9 || g.rows !== 1) { console.error("Bad grid:", g); process.exit(1); }\n'
            f'const sl = sliceSpriteSheet(raw, {sp.width}, {sp.height}, g.cols, g.rows, true, "1-4");\n'
            'if (!sl || sl.activeIndices.length !== 4) { console.error("Bad slice"); process.exit(1); }\n'
            'const gif = encodeGif89a(sl.activeIndices.map(i => sl.frames[i].rgba), sl.tw, sl.th, 12, true);\n'
            f'fs.writeFileSync({json.dumps(os.path.join(OUT_DIR, "pb_sliced_sheet.gif"))}, Buffer.from(gif));\n'
            'console.log("VERIFIED Sprite Sheet Slicer: detected 9x1 strip, sliced frames 1-4 ->", gif.length, "bytes GIF");\n'
        )
        r_sl = subprocess.run(["node", "-e", slice_test], cwd=ROOT, text=True)
        os.remove(sp_bin)
        if r_sl.returncode != 0:
            failures.append("polar_bear 9x1 sprite sheet slicer failed")

    if failures:
        print("\nBENCHMARK FAILURES:", failures)
        sys.exit(1)
    print("\nALL 5 MASCOTS + SPRITE SHEET SLICER PASSED EMPIRICAL BENCHMARK!")


if __name__ == "__main__":
    main()
