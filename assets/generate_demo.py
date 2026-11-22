import base64
import io
import math
import os
import re
import shutil
import subprocess
import tempfile
from PIL import Image, ImageDraw, ImageFilter, ImageFont

# --- Output Spec (960x540 16:9, exact 25 FPS / 40ms GIF delay) ---
OUT_W, OUT_H = 960, 540
S = 2  # 2x Retina master stage (1920x1080) -> crisp vector chrome & lossless optical zoom
W, H = OUT_W * S, OUT_H * S
FPS = 25
TOTAL_FRAMES = 290  # 11.6s across 3 clean scenarios

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
CRATE_DIR = os.path.dirname(SCRIPT_DIR)
OUT_GIF = os.path.join(CRATE_DIR, "brand", "demo.gif")
OUT_MP4 = os.path.join(CRATE_DIR, "brand", "demo.mp4")

LANCZOS = getattr(Image, "Resampling", Image).LANCZOS
NEAREST = getattr(Image, "Resampling", Image).NEAREST
MEDIANCUT = getattr(Image, "Quantize", Image).MEDIANCUT
DITHER_NONE = getattr(Image, "Dither", Image).NONE

# --- Exact Verified macOS Sonoma/Sequoia + VS Code Dark Modern Tokens ---
C_BG_EDGE = (14, 17, 24)        # #0E1118 deep studio slate
C_BG_GLOW = (26, 35, 56)        # #1A2338 midnight studio radial glow
C_BG_WARM = (42, 28, 24)        # #2A1C18 faint warm glow behind Dock

C_MENUBAR = (28, 28, 30)        # #1C1C1E macOS dark menu bar (24pt)
C_TERM_BG = (30, 30, 30)        # #1E1E1E macOS Terminal Dark
C_TERM_BAR = (46, 46, 48)       # #2E2E30 macOS 28pt titlebar
C_TERM_BAR_DIM = (38, 38, 41)   # #262629 unfocused titlebar

# VS Code Dark Modern (Official Default Theme)
C_VSC_CHROME = (24, 24, 24)     # #181818 titlebar, activity bar, sidebar
C_VSC_EDITOR = (31, 31, 31)     # #1F1F1F editor canvas
C_VSC_BORDER = (43, 43, 43)     # #2B2B2B panel hairlines
C_VSC_SEL = (55, 55, 61)        # #37373D active explorer row (solid pre-blended!)
C_VSC_STATUS = (0, 120, 212)    # #0078D4 official VS Code status bar blue
C_VSC_ACT_ICON = (204, 204, 204)
C_VSC_DIM_ICON = (110, 110, 110)

# Exact Apple HIG Traffic Lights (12pt diameter, 8pt gap -> 20pt pitch)
C_TL_RED = (255, 95, 87)        # #FF5F57
C_TL_YELLOW = (254, 188, 46)    # #FEBC2E
C_TL_GREEN = (40, 200, 64)      # #28C840
C_TL_OFF = (82, 84, 88)

C_ACCENT_BLUE = (10, 132, 255)  # #0A84FF macOS Dark Accent Blue
C_TEXT = (232, 237, 244)        # #E8EDF4
C_MUTED = (140, 148, 162)       # #8C94A2
C_GREEN = (63, 185, 80)         # #3FB950
C_CYAN = (121, 192, 255)        # #79C0FF
C_KW = (197, 134, 192)          # #C586C0 VS Code keyword
C_FN = (220, 220, 170)          # #DCDCAA VS Code function
C_TYPE = (78, 201, 176)         # #4EC9B0 VS Code type teal
C_STR = (206, 145, 120)         # #CE9178 VS Code string
C_CM = (106, 153, 85)           # #6A9955 VS Code comment
C_AMBER = (240, 142, 66)        # #F08E42 DIYPets warm amber
C_BUBBLE_BG = (22, 26, 36)      # #161A24 speech bubble fill
C_HEART = (240, 60, 98)         # #F03C62 pixel heart
C_HEART_HI = (255, 198, 212)    # #FFC6D4
C_BALL = (242, 104, 62)         # #F2683E
C_BALL_STRIPE = (255, 224, 128) # #FFE080

# Exact Logical Layout (1x pt; 106pt open stage between windows and Dock!)
MENUBAR_H = 24
TITLEBAR_H = 28
WIN_RADIUS = 10
L_BOX = (24, 78, 472, 378)      # 448 x 300pt macOS Terminal
R_BOX = (488, 78, 936, 378)     # 448 x 300pt VS Code Dark Modern
DOCK_BOX = (318, 484, 642, 534) # 324 x 50pt floating frosted-glass Dock (6pt above bottom)
DOCK_TOP = DOCK_BOX[1]          # 484pt
FLOOR_Y = 534

PROMPT = "sm@mac diypets % "
CMD_REACT = 'diypets react --mood happy --text "Build passed!"'


def get_font(pt, bold=False, mono=False):
    px = int(round(pt * S))
    if mono:
        cands = (
            ["C:\\Windows\\Fonts\\consolab.ttf", "C:\\Windows\\Fonts\\cascadiamono.ttf", "DejaVuSansMono-Bold.ttf"]
            if bold else
            ["C:\\Windows\\Fonts\\consola.ttf", "C:\\Windows\\Fonts\\cascadiamono.ttf", "DejaVuSansMono.ttf"]
        )
    else:
        cands = (
            ["C:\\Windows\\Fonts\\segoeuib.ttf", "C:\\Windows\\Fonts\\arialbd.ttf", "DejaVuSans-Bold.ttf"]
            if bold else
            ["C:\\Windows\\Fonts\\segoeui.ttf", "C:\\Windows\\Fonts\\arial.ttf", "DejaVuSans.ttf"]
        )
    for fp in cands:
        try:
            return ImageFont.truetype(fp, px)
        except OSError:
            continue
    return ImageFont.load_default()


F_MENU = get_font(11.5)
F_MENU_B = get_font(11.5, bold=True)
F_TITLE = get_font(11.5, bold=True)
F_UI = get_font(11)
F_UI_B = get_font(11, bold=True)
F_XS = get_font(9.5)
F_XS_B = get_font(9.5, bold=True)
F_MONO = get_font(9.6, mono=True)
F_MONO_B = get_font(9.6, bold=True, mono=True)
F_CODE = get_font(10.0, mono=True)
F_BUBBLE = get_font(12.5, bold=True, mono=True)


def clamp(v, lo=0.0, hi=1.0):
    return max(lo, min(hi, v))


def lerp(a, b, t):
    return a + (b - a) * t


def ease_out_cubic(t):
    t = clamp(t)
    return 1.0 - (1.0 - t) ** 3


def smootherstep(t):
    t = clamp(t)
    return t * t * t * (t * (t * 6.0 - 15.0) + 10.0)


def draw_centered(d, cx, cy, txt, font, fill):
    bb = d.textbbox((0, 0), txt, font=font)
    tw, th = bb[2] - bb[0], bb[3] - bb[1]
    d.text((cx - tw / 2.0 - bb[0], cy - th / 2.0 - bb[1]), txt, font=font, fill=fill)


def load_pack_anims(pack_id, target_h_pt):
    xp = os.path.join(CRATE_DIR, "assets", pack_id, "deskpet.xml")
    txt = open(xp, "r", encoding="utf-8").read()
    m = re.search(r"<Png[^>]*>([\s\S]*?)</Png>", txt, re.I)
    tx = int(re.search(r"<TilesX>(\d+)</TilesX>", txt, re.I).group(1))
    ty = int(re.search(r"<TilesY>(\d+)</TilesY>", txt, re.I).group(1))
    gy_m = re.search(r"<GroundY>(\d+)</GroundY>", txt, re.I)
    sheet = Image.open(io.BytesIO(base64.b64decode(m.group(1).strip()))).convert("RGBA")
    tw, th = sheet.width // tx, sheet.height // ty
    gy = int(gy_m.group(1)) if gy_m else th

    def anim_ids(aid, fallback):
        ma = re.search(rf'<Animation id="{aid}">[\s\S]*?</Animation>', txt, re.I)
        if not ma:
            return fallback
        ids = [int(x) for x in re.findall(r"<Frame>(\d+)</Frame>", ma.group(0))]
        return ids[:4] if ids else fallback

    walk_ids = anim_ids("1", [0, 1, 2, 3])
    drag_ids = anim_ids("4", [walk_ids[0]])
    fall_ids = anim_ids("5", [walk_ids[0]])

    target_h_px = target_h_pt * S
    scale = target_h_px / float(th)
    nw = max(1, round(tw * scale))
    nh = max(1, round(th * scale))
    gy_pt = gy * (target_h_pt / float(th))

    def build_set(ids):
        lefts, rights = [], []
        for idx in ids:
            cx, cy = (idx % tx) * tw, (idx // tx) * th
            tile = sheet.crop((cx, cy, cx + tw, cy + th))
            res = tile.resize((nw, nh), NEAREST)
            lefts.append(res)
            rights.append(res.transpose(Image.Transpose.FLIP_LEFT_RIGHT))
        return lefts, rights

    wl, wr = build_set(walk_ids)
    dl, dr = build_set(drag_ids)
    fl, fr = build_set(fall_ids)
    return {
        "walk_l": wl, "walk_r": wr,
        "drag_l": dl, "drag_r": dr,
        "fall_l": fl, "fall_r": fr,
        "w_pt": nw / float(S), "h_pt": nh / float(S),
        "gy_pt": gy_pt,
        "sample_tile": wl[0],
    }


def draw_traffic_lights(d, x0_pt, cy_pt, active=True):
    cols = (C_TL_RED, C_TL_YELLOW, C_TL_GREEN) if active else (C_TL_OFF, C_TL_OFF, C_TL_OFF)
    r = 6 * S
    for i, col in enumerate(cols):
        cx = (x0_pt + i * 20) * S
        cy = cy_pt * S
        d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(*col, 255))
        if active:
            rim = tuple(max(0, c - 36) for c in col)
            d.ellipse([cx - r, cy - r, cx + r, cy + r], outline=(*rim, 255), width=1)


# --- 4x Supersampled Authentic macOS Icons (Strictly Masked to 22.5% Squircle!) ---
def make_apple_logo(h_px):
    """Clean, symmetric 4x supersampled Apple silhouette for macOS menu bar."""
    hi = Image.new("RGBA", (128, 128), (0, 0, 0, 0))
    d = ImageDraw.Draw(hi, "RGBA")
    col = (236, 240, 246, 245)
    # Two overlapping apple lobes + smooth central bridge
    d.ellipse([24, 38, 72, 114], fill=col)
    d.ellipse([56, 38, 104, 114], fill=col)
    d.rectangle([44, 48, 84, 104], fill=col)
    # Bottom dip
    d.ellipse([52, 106, 76, 122], fill=(0, 0, 0, 0))
    # Top stem dip
    d.ellipse([52, 32, 76, 48], fill=(0, 0, 0, 0))
    # Iconic right bite
    d.ellipse([86, 50, 122, 92], fill=(0, 0, 0, 0))
    # Crisp tilted leaf above top dip
    d.pieslice([60, 8, 92, 42], start=110, end=290, fill=col)
    d.ellipse([66, 14, 86, 34], fill=col)
    return hi.resize((h_px, h_px), LANCZOS)


def make_squircle_icon(out_px, render_hi_fn):
    """Draw at 256x256, apply exact 22.5% macOS squircle mask + specular rim, downsample to out_px."""
    hi_s = 256
    rad = round(hi_s * 0.225)
    canvas = Image.new("RGBA", (hi_s, hi_s), (0, 0, 0, 0))
    render_hi_fn(canvas, hi_s)

    mask = Image.new("L", (hi_s, hi_s), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, hi_s - 1, hi_s - 1], radius=rad, fill=255)
    clipped = Image.new("RGBA", (hi_s, hi_s), (0, 0, 0, 0))
    clipped.paste(canvas, (0, 0), mask)

    # Subtle macOS specular top rim (proper alpha_composite so it never overwrites!)
    rim = Image.new("RGBA", (hi_s, hi_s), (0, 0, 0, 0))
    ImageDraw.Draw(rim).rounded_rectangle([3, 3, hi_s - 4, hi_s - 4], radius=rad - 2, outline=(255, 255, 255, 58), width=4)
    clipped.alpha_composite(rim)
    return clipped.resize((out_px, out_px), LANCZOS)


def fill_vgrad(im, top_rgb, bot_rgb):
    w, h = im.size
    d = ImageDraw.Draw(im)
    for y in range(h):
        t = y / float(max(1, h - 1))
        col = tuple(round(lerp(top_rgb[c], bot_rgb[c], t)) for c in range(3))
        d.line([(0, y), (w, y)], fill=(*col, 255))


def build_og_dock_icons():
    sz = 36 * S  # 72px Retina

    # 1. Official macOS Finder Icon (two-tone blue/ice face + curved center nose line + smile)
    def r_finder(im, s):
        fill_vgrad(im, (36, 158, 255), (18, 96, 224))
        # Right pale-ice half with subtle curved nose contour
        right = Image.new("RGBA", (s, s), (0, 0, 0, 0))
        fill_vgrad(right, (238, 248, 255), (194, 226, 252))
        rmask = Image.new("L", (s, s), 0)
        rd = ImageDraw.Draw(rmask)
        rd.polygon([(134, 0), (s, 0), (s, s), (128, s), (128, 152), (104, 152), (118, 86), (134, 0)], fill=255)
        im.paste(right, (0, 0), rmask)
        d = ImageDraw.Draw(im, "RGBA")
        # Nose contour hairline
        d.line([(134, 0), (118, 86), (106, 148), (134, 148), (128, s)], fill=(22, 76, 158, 165), width=5, joint="curve")
        # Crisp dark eyes
        d.rounded_rectangle([62, 68, 78, 108], radius=8, fill=(24, 34, 52, 255))
        d.rounded_rectangle([176, 68, 192, 108], radius=8, fill=(24, 34, 52, 255))
        # Iconic wide Finder smile
        d.arc([48, 108, 208, 206], start=20, end=160, fill=(24, 34, 52, 255), width=12)

    # 2. Official macOS Terminal Icon (metallic dark frame + recessed black screen + crisp >_)
    def r_terminal(im, s):
        fill_vgrad(im, (108, 112, 120), (62, 66, 74))
        d = ImageDraw.Draw(im, "RGBA")
        d.rounded_rectangle([18, 18, s - 18, s - 18], radius=36, fill=(24, 25, 29, 255))
        d.line([(54, 72), (96, 106), (54, 140)], fill=(242, 246, 252, 255), width=16, joint="curve")
        d.line([(106, 144), (160, 144)], fill=(242, 246, 252, 255), width=16)

    # 3. Official macOS VS Code Icon (clean white squircle + authentic 3-shade folded blue ribbon)
    def r_vscode(im, s):
        fill_vgrad(im, (250, 252, 255), (224, 232, 244))
        d = ImageDraw.Draw(im, "RGBA")
        # Back diagonal ribbon (deepest blue #0065A9)
        d.polygon([(46, 94), (176, 40), (192, 66), (62, 174), (42, 154)], fill=(0, 101, 169, 255))
        # Front diagonal ribbon (mid blue #007ACC)
        d.polygon([(46, 162), (176, 216), (192, 190), (62, 82), (42, 102)], fill=(0, 122, 204, 255))
        # Right vertical anchor bar (bright cyan-blue #1F9CF0)
        d.polygon([(170, 38), (212, 58), (212, 198), (170, 218)], fill=(31, 156, 240, 255))

    # 4. DIYPets Highland Cow App Icon (warm sunlit squircle + your real Highland Cow)
    def r_diypets(im, s):
        fill_vgrad(im, (255, 248, 236), (246, 224, 192))
        cow_p = os.path.join(CRATE_DIR, "brand", "logo-512.png")
        if os.path.isfile(cow_p):
            ci = Image.open(cow_p).convert("RGBA").resize((s - 34, s - 34), LANCZOS)
            im.alpha_composite(ci, (17, 17))

    # 5. Official macOS Safari Icon (white squircle + blue compass dial + radial ticks + 45-deg red/white needle)
    def r_safari(im, s):
        fill_vgrad(im, (252, 253, 255), (228, 234, 242))
        # Circular blue gradient dial
        dial = Image.new("RGBA", (s, s), (0, 0, 0, 0))
        fill_vgrad(dial, (34, 190, 255), (14, 106, 238))
        dmask = Image.new("L", (s, s), 0)
        ImageDraw.Draw(dmask).ellipse([24, 24, s - 24, s - 24], fill=255)
        im.paste(dial, (0, 0), dmask)
        d = ImageDraw.Draw(im, "RGBA")
        # Radial compass ticks
        cx, cy = s / 2.0, s / 2.0
        for deg in range(0, 360, 15):
            ang = math.radians(deg)
            r_in = 84 if (deg % 45 == 0) else 92
            r_out = 100
            d.line(
                [(cx + math.cos(ang) * r_in, cy + math.sin(ang) * r_in),
                 (cx + math.cos(ang) * r_out, cy + math.sin(ang) * r_out)],
                fill=(255, 255, 255, 210),
                width=3 if (deg % 45 == 0) else 2,
            )
        # Authentic 45-degree diamond compass needle (NE red #FF3B30, SW white #FFFFFF)
        ne = (cx + 62, cy - 62)
        sw = (cx - 62, cy + 62)
        nw = (cx - 15, cy - 15)
        se = (cx + 15, cy + 15)
        d.polygon([ne, se, nw], fill=(255, 59, 48, 255))
        d.polygon([sw, nw, se], fill=(248, 250, 255, 255))
        d.ellipse([cx - 6, cy - 6, cx + 6, cy + 6], fill=(220, 226, 234, 255))

    # 6. Official macOS Sequoia Blue Folder Icon
    def r_folder(im, s):
        # Clean dark-glass icon pad with authentic macOS folder inside
        fill_vgrad(im, (38, 46, 62), (24, 30, 42))
        d = ImageDraw.Draw(im, "RGBA")
        # Back folder tab
        d.rounded_rectangle([34, 62, 122, 108], radius=14, fill=(58, 152, 232, 255))
        d.rounded_rectangle([34, 78, s - 34, s - 46], radius=16, fill=(64, 160, 238, 255))
        # Front folder flap (brighter sky blue + top highlight line)
        d.rounded_rectangle([34, 96, s - 34, s - 42], radius=16, fill=(108, 196, 252, 255))
        d.line([(48, 102), (s - 48, 102)], fill=(168, 224, 255, 255), width=4)

    return [
        (make_squircle_icon(sz, r_finder), True),
        (make_squircle_icon(sz, r_terminal), True),
        (make_squircle_icon(sz, r_vscode), True),
        (make_squircle_icon(sz, r_diypets), True),
        None,  # Dock separator
        (make_squircle_icon(sz, r_safari), False),
        (make_squircle_icon(sz, r_folder), False),
    ]


def build_static_stage(dock_icons):
    # 1. Smooth studio wallpaper
    sw, sh = 240, 135
    wp = Image.new("RGB", (sw, sh), C_BG_EDGE)
    px = wp.load()
    for y in range(sh):
        for x in range(sw):
            d1 = math.hypot((x - sw * 0.50) / (sw * 0.58), (y - sh * 0.28) / (sh * 0.55))
            w1 = max(0.0, 1.0 - d1) ** 2
            d2 = math.hypot((x - sw * 0.50) / (sw * 0.44), (y - sh * 0.96) / (sh * 0.40))
            w2 = max(0.0, 1.0 - d2) ** 2 * 0.55
            r = clamp(C_BG_EDGE[0] + (C_BG_GLOW[0] - C_BG_EDGE[0]) * w1 + (C_BG_WARM[0] - C_BG_EDGE[0]) * w2, 0, 255)
            g = clamp(C_BG_EDGE[1] + (C_BG_GLOW[1] - C_BG_EDGE[1]) * w1 + (C_BG_WARM[1] - C_BG_EDGE[1]) * w2, 0, 255)
            b = clamp(C_BG_EDGE[2] + (C_BG_GLOW[2] - C_BG_EDGE[2]) * w1 + (C_BG_WARM[2] - C_BG_EDGE[2]) * w2, 0, 255)
            px[x, y] = (round(r), round(g), round(b))
    bg = wp.resize((W, H), LANCZOS).convert("RGBA")

    # 2. Translucent macOS Menu Bar (exact 24pt = 48px)
    mb_ov = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    mb_d = ImageDraw.Draw(mb_ov, "RGBA")
    mb_d.rectangle([0, 0, W, MENUBAR_H * S], fill=(*C_MENUBAR, 218))
    mb_d.line([(0, MENUBAR_H * S - 1), (W, MENUBAR_H * S - 1)], fill=(255, 255, 255, 24), width=2)
    bg.alpha_composite(mb_ov)

    # Crisp 4x Apple Logo in Menu Bar
    apple_ico = make_apple_logo(15 * S)
    bg.alpha_composite(apple_ico, (12 * S, 4 * S))

    d = ImageDraw.Draw(bg, "RGBA")
    d.text((36 * S, 5 * S), "DIYPets", font=F_MENU_B, fill=C_TEXT)
    d.text((94 * S, 5 * S), "Herd", font=F_MENU, fill=C_MUTED)
    d.text((134 * S, 5 * S), "Studio", font=F_MENU, fill=C_MUTED)
    d.text((182 * S, 5 * S), "Play", font=F_MENU, fill=C_MUTED)
    d.text((220 * S, 5 * S), "Window", font=F_MENU, fill=C_MUTED)

    # Right menu bar status icons
    cow_p = os.path.join(CRATE_DIR, "brand", "logo-512.png")
    if os.path.isfile(cow_p):
        tray_cow = Image.open(cow_p).convert("RGBA").resize((15 * S, 15 * S), LANCZOS)
        bg.alpha_composite(tray_cow, (814 * S, 4 * S))
    # Wi-Fi arcs
    wx, wy = 842 * S, 16 * S
    for rad in (6, 12, 18):
        d.arc([wx - rad, wy - rad, wx + rad, wy + rad], start=225, end=315, fill=(224, 230, 240, 225), width=3)
    # Battery icon
    bx, by = 860 * S, 8 * S
    d.rounded_rectangle([bx, by, bx + 22 * S, by + 9 * S], radius=4, outline=(224, 230, 240, 200), width=2)
    d.rounded_rectangle([bx + 4, by + 4, bx + 18 * S, by + 9 * S - 4], radius=2, fill=(224, 230, 240, 235))
    d.rectangle([bx + 22 * S + 1, by + 6, bx + 22 * S + 4, by + 9 * S - 6], fill=(224, 230, 240, 200))
    d.text((890 * S, 5 * S), "10:42 AM", font=F_MENU, fill=C_TEXT)

    # 3. Real Soft Gaussian Drop Shadows for Both Windows
    shadow = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    sd = ImageDraw.Draw(shadow, "RGBA")
    for x0, y0, x1, y1 in (L_BOX, R_BOX):
        sd.rounded_rectangle([x0 * S - 6, y0 * S + 16, x1 * S + 6, y1 * S + 24], radius=WIN_RADIUS * S + 4, fill=(0, 0, 0, 150))
    bg.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(radius=22)))

    # 4. Right Window: Authentic VS Code Dark Modern (Activity Bar + Explorer + Tab + Real DIYPets Code)
    rx0, ry0, rx1, ry1 = [v * S for v in R_BOX]
    rad = WIN_RADIUS * S
    th = TITLEBAR_H * S
    act_w = 28 * S
    sb_w = 96 * S
    tab_h = 22 * S
    st_h = 18 * S

    vsc = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    vd = ImageDraw.Draw(vsc, "RGBA")
    vd.rounded_rectangle([rx0, ry0, rx1, ry1], radius=rad, fill=(*C_VSC_EDITOR, 255))
    # Titlebar
    vd.rounded_rectangle([rx0, ry0, rx1, ry0 + th], radius=rad, fill=(*C_VSC_CHROME, 255))
    vd.rectangle([rx0, ry0 + th - rad, rx1, ry0 + th], fill=(*C_VSC_CHROME, 255))
    vd.line([(rx0, ry0 + th), (rx1, ry0 + th)], fill=(*C_VSC_BORDER, 255), width=2)
    draw_traffic_lights(vd, R_BOX[0] + 18, R_BOX[1] + 14, active=True)
    # Centered VS Code Command Center pill
    vd.rounded_rectangle([(rx0 + rx1) / 2 - 140, ry0 + 10, (rx0 + rx1) / 2 + 140, ry0 + th - 10], radius=10, fill=(38, 38, 40, 255), outline=(62, 62, 66, 255), width=2)
    draw_centered(vd, (rx0 + rx1) / 2, ry0 + th / 2, "diypets — Visual Studio Code", F_XS, C_MUTED)

    # Left Activity Bar + Sidebar (#181818)
    vd.rectangle([rx0, ry0 + th + 2, rx0 + act_w + sb_w, ry1 - st_h], fill=(*C_VSC_CHROME, 255))
    vd.line([(rx0 + act_w, ry0 + th), (rx0 + act_w, ry1 - st_h)], fill=(*C_VSC_BORDER, 255), width=2)
    vd.line([(rx0 + act_w + sb_w, ry0 + th), (rx0 + act_w + sb_w, ry1 - st_h)], fill=(*C_VSC_BORDER, 255), width=2)

    # Activity Bar active blue rail + crisp minimalist glyphs
    vd.rectangle([rx0 + 2, ry0 + th + 14, rx0 + 6, ry0 + th + 52], fill=(*C_VSC_STATUS, 255))
    # Explorer icon (2 overlapping pages)
    vd.rectangle([rx0 + 18, ry0 + th + 20, rx0 + 36, ry0 + th + 44], outline=(*C_VSC_ACT_ICON, 255), width=3)
    vd.rectangle([rx0 + 24, ry0 + th + 26, rx0 + 42, ry0 + th + 50], fill=(*C_VSC_CHROME, 255), outline=(*C_VSC_ACT_ICON, 255), width=3)
    # Search magnifier icon
    vd.ellipse([rx0 + 18, ry0 + th + 76, rx0 + 36, ry0 + th + 94], outline=(*C_VSC_DIM_ICON, 255), width=3)
    vd.line([(rx0 + 34, ry0 + th + 92), (rx0 + 42, ry0 + th + 100)], fill=(*C_VSC_DIM_ICON, 255), width=3)
    # Git branch icon
    vd.ellipse([rx0 + 20, ry0 + th + 124, rx0 + 30, ry0 + th + 134], outline=(*C_VSC_DIM_ICON, 255), width=3)
    vd.ellipse([rx0 + 20, ry0 + th + 146, rx0 + 30, ry0 + th + 156], outline=(*C_VSC_DIM_ICON, 255), width=3)
    vd.line([(rx0 + 25, ry0 + th + 134), (rx0 + 25, ry0 + th + 146)], fill=(*C_VSC_DIM_ICON, 255), width=3)

    # Explorer Tree (authentic #37373D selection bar — NO white box bug!)
    ex = rx0 + act_w
    tree = [
        ("EXPLORER", C_MUTED, True, False),
        ("v diypets", C_TEXT, False, False),
        ("  v src", C_TEXT, False, False),
        ("    agent_api.rs", C_MUTED, False, False),
        ("    bubble.rs", C_MUTED, False, False),
        ("    diypets.rs", C_TEXT, True, True),
        ("    settings.rs", C_MUTED, False, False),
        ("  v assets", C_TEXT, False, False),
        ("    highland_cow", C_MUTED, False, False),
        ("    polar_bear", C_MUTED, False, False),
    ]
    for i, (lbl, col, is_b, is_sel) in enumerate(tree):
        ty = ry0 + th + 16 + i * 38
        if is_sel:
            vd.rectangle([ex + 2, ty - 5, ex + sb_w - 2, ty + 27], fill=(*C_VSC_SEL, 255))
        vd.text((ex + 14, ty), lbl, font=F_XS_B if is_b else F_XS, fill=col)

    # Editor Tab Strip
    ed_x = ex + sb_w + 2
    vd.rectangle([ed_x, ry0 + th + 2, rx1 - 2, ry0 + th + tab_h], fill=(*C_VSC_CHROME, 255))
    vd.rectangle([ed_x, ry0 + th + 2, ed_x + 170, ry0 + th + tab_h], fill=(*C_VSC_EDITOR, 255))
    vd.rectangle([ed_x, ry0 + th + 2, ed_x + 170, ry0 + th + 6], fill=(*C_VSC_STATUS, 255))
    vd.text((ed_x + 18, ry0 + th + 12), "diypets.rs", font=F_XS_B, fill=C_TEXT)
    vd.text((ed_x + 190, ry0 + th + 12), "agent_api.rs", font=F_XS, fill=C_MUTED)
    vd.line([(ed_x, ry0 + th + tab_h), (rx1, ry0 + th + tab_h)], fill=(*C_VSC_BORDER, 255), width=2)

    # Authentic real DIYPets Rust code inside Editor
    code_lines = [
        [(C_CM, "// Pure Rust mascot engine (~7.8 MB RAM)")],
        [(C_KW, "let "), (C_TEXT, "(tx, rx) = mpsc::"), (C_FN, "channel"), (C_TEXT, "();")],
        [(C_TEXT, "agent_api::"), (C_FN, "start_local_server"), (C_TEXT, "(tx);")],
        [(C_KW, "let "), (C_TEXT, "cfg = "), (C_TYPE, "Settings"), (C_TEXT, "::"), (C_FN, "load_sanitized"), (C_TEXT, "(&p);")],
        [(C_TEXT, "")],
        [(C_KW, "for "), (C_TEXT, "pet "), (C_KW, "in "), (C_TEXT, "&"), (C_KW, "mut "), (C_TEXT, "self.pets {")],
        [(C_TEXT, "    pet.state."), (C_FN, "tick_in_bounds"), (C_TEXT, "(dt, y, l, r);")],
        [(C_TEXT, "    self."), (C_FN, "step_play_mode"), (C_TEXT, "(el, now);")],
        [(C_TEXT, "}")],
        [(C_CM, "// MCP: http://127.0.0.1:3721/mcp")],
    ]
    for li, segs in enumerate(code_lines):
        cy = ry0 + th + tab_h + 16 + li * 38
        vd.text((ed_x + 14, cy), f"{li + 1:>2}", font=F_CODE, fill=(104, 110, 122))
        cx = ed_x + 54
        for col, seg in segs:
            vd.text((cx, cy), seg, font=F_CODE, fill=col)
            bb = vd.textbbox((0, 0), seg, font=F_CODE)
            cx += bb[2] - bb[0]

    # Official VS Code Status Bar (#0078D4) cleanly clipped to window bottom corners
    vd.rounded_rectangle([rx0, ry1 - st_h - rad, rx1, ry1], radius=rad, fill=(*C_VSC_STATUS, 255))
    vd.rectangle([rx0, ry1 - st_h - rad, rx1, ry1 - st_h], fill=(*C_VSC_EDITOR, 255))
    vd.rectangle([rx0, ry1 - st_h - rad, rx0 + act_w + sb_w, ry1 - st_h], fill=(*C_VSC_CHROME, 255))
    vd.text((rx0 + 18, ry1 - st_h + 6), "main*   0 errors", font=F_XS_B, fill=(255, 255, 255))
    vd.text((rx1 - 244, ry1 - st_h + 6), "MCP :3721   7.8 MB RAM", font=F_XS_B, fill=(224, 240, 255))
    vd.rounded_rectangle([rx0, ry0, rx1, ry1], radius=rad, outline=(255, 255, 255, 34), width=2)
    bg.alpha_composite(vsc)

    # 5. Floating Frosted-Glass macOS Dock (drawn via alpha_composite -> true translucent glass!)
    dx0, dy0, dx1, dy1 = [v * S for v in DOCK_BOX]
    d_rad = 16 * S
    dock_sh = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(dock_sh).rounded_rectangle([dx0 - 6, dy0 + 8, dx1 + 6, dy1 + 14], radius=d_rad, fill=(0, 0, 0, 125))
    bg.alpha_composite(dock_sh.filter(ImageFilter.GaussianBlur(radius=14)))

    dock_ov = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    dd = ImageDraw.Draw(dock_ov, "RGBA")
    dd.rounded_rectangle([dx0, dy0, dx1, dy1], radius=d_rad, fill=(44, 48, 60, 192), outline=(255, 255, 255, 58), width=2)
    bg.alpha_composite(dock_ov)

    ix = dx0 + 22
    iy = dy0 + 12
    d_top = ImageDraw.Draw(bg, "RGBA")
    for entry in dock_icons:
        if entry is None:
            sep_ov = Image.new("RGBA", (W, H), (0, 0, 0, 0))
            ImageDraw.Draw(sep_ov).line([(ix + 4, dy0 + 18), (ix + 4, dy1 - 18)], fill=(255, 255, 255, 54), width=2)
            bg.alpha_composite(sep_ov)
            ix += 20
            continue
        icon_im, is_running = entry
        bg.alpha_composite(icon_im, (ix, iy))
        if is_running:
            dot_x = ix + icon_im.width // 2
            dot_y = dy1 - 9
            d_top.ellipse([dot_x - 3, dot_y - 3, dot_x + 3, dot_y + 3], fill=(235, 240, 248, 235))
        ix += icon_im.width + 18

    return bg


def draw_terminal_window(frame, state):
    lx0, ly0, lx1, ly1 = [v * S for v in L_BOX]
    rad = WIN_RADIUS * S
    th = TITLEBAR_H * S
    focused = state["term_focused"]

    ov = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(ov, "RGBA")
    d.rounded_rectangle([lx0, ly0, lx1, ly1], radius=rad, fill=(*C_TERM_BG, 255))
    tbar = C_TERM_BAR if focused else C_TERM_BAR_DIM
    d.rounded_rectangle([lx0, ly0, lx1, ly0 + th], radius=rad, fill=(*tbar, 255))
    d.rectangle([lx0, ly0 + th - rad, lx1, ly0 + th], fill=(*tbar, 255))
    d.line([(lx0, ly0 + th), (lx1, ly0 + th)], fill=(255, 255, 255, 22), width=2)
    draw_traffic_lights(d, L_BOX[0] + 18, L_BOX[1] + 14, active=focused)
    draw_centered(d, (lx0 + lx1) / 2, ly0 + th / 2, "zsh — diypets", F_TITLE, C_TEXT if focused else C_MUTED)

    cx = lx0 + 26
    y = ly0 + th + 24
    pw = d.textbbox((0, 0), PROMPT, font=F_MONO)[2]

    d.text((cx, y), PROMPT, font=F_MONO, fill=C_CYAN)
    d.text((cx + pw, y), "cargo install diypets", font=F_MONO, fill=C_TEXT)
    y += 38
    d.text((cx, y), "  Installed diypets v0.1.6 (7.8 MB RAM)", font=F_MONO, fill=C_GREEN)
    y += 44

    d.text((cx, y), PROMPT, font=F_MONO, fill=C_CYAN)
    d.text((cx + pw, y), "diypets &", font=F_MONO, fill=C_TEXT)
    y += 38
    d.text((cx, y), "[mcp]  http://127.0.0.1:3721/mcp ready", font=F_MONO, fill=C_MUTED)
    y += 34
    d.text((cx, y), "[herd] 4 active: cow · dog · bear · stego", font=F_MONO, fill=C_MUTED)
    y += 48

    # Clean single-line command typing (never wraps awkwardly!)
    shown = CMD_REACT[:state["typed_chars"]]
    d.text((cx, y), PROMPT, font=F_MONO, fill=C_CYAN)
    d.text((cx + pw, y), shown, font=F_MONO, fill=C_TEXT)
    if focused and not state["react_done"]:
        tw = d.textbbox((0, 0), shown, font=F_MONO)[2] if shown else 0
        d.rectangle([cx + pw + tw + 2, y + 2, cx + pw + tw + 12, y + 22], fill=(*C_TEXT, 235))
    y += 40

    if state["react_done"]:
        d.text((cx, y), 'OK  DIYPets reacted: happy ("Build passed!")', font=F_MONO_B, fill=C_GREEN)
        y += 38

    if state["ball_dropped"]:
        d.text((cx, y), '[play] bouncy ball dropped · herd chasing!', font=F_MONO_B, fill=C_AMBER)

    d.rounded_rectangle([lx0, ly0, lx1, ly1], radius=rad, outline=(255, 255, 255, 44 if focused else 26), width=2)
    frame.alpha_composite(ov)


def draw_macos_context_menu(frame, x_pt, y_pt, hovered_idx, pressed=False):
    items = [
        "Headpat / Hop",
        "Drop Bouncy Ball",
        "Follow Cursor",
        "Sleep",
        "---",
        "Application Settings",
        "DIYPets Studio",
    ]
    mw = 166 * S
    row_h = 23 * S
    sep_h = 9 * S
    pad = 6 * S
    mh = pad * 2 + sum(sep_h if it == "---" else row_h for it in items)

    x0 = int(clamp(x_pt * S, 24, W - mw - 24))
    y0 = int(clamp(y_pt * S - mh, MENUBAR_H * S + 12, H - mh - 24))

    sh = Image.new("RGBA", (mw + 40, mh + 40), (0, 0, 0, 0))
    ImageDraw.Draw(sh).rounded_rectangle([20, 24, 20 + mw, 24 + mh], radius=16, fill=(0, 0, 0, 145))
    frame.alpha_composite(sh.filter(ImageFilter.GaussianBlur(radius=10)), (x0 - 20, y0 - 20))

    ov = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(ov, "RGBA")
    d.rounded_rectangle([x0, y0, x0 + mw, y0 + mh], radius=16, fill=(38, 41, 48, 244), outline=(255, 255, 255, 52), width=2)

    cy = y0 + pad
    for idx, lbl in enumerate(items):
        if lbl == "---":
            d.line([(x0 + 18, cy + sep_h // 2), (x0 + mw - 18, cy + sep_h // 2)], fill=(255, 255, 255, 34), width=2)
            cy += sep_h
            continue
        if idx == hovered_idx:
            col = (52, 158, 255, 255) if pressed else (*C_ACCENT_BLUE, 255)
            d.rounded_rectangle([x0 + 10, cy + 2, x0 + mw - 10, cy + row_h - 2], radius=9, fill=col)
            d.text((x0 + 24, cy + 8), lbl, font=F_UI_B, fill=(255, 255, 255, 255))
        else:
            d.text((x0 + 24, cy + 8), lbl, font=F_UI, fill=C_TEXT)
        cy += row_h
    frame.alpha_composite(ov)
    return (x0 + mw * 0.5) / S, (y0 + pad + row_h * 1.5) / S


def make_cursor_2x():
    cw, ch = 44, 52
    hi = Image.new("RGBA", (cw * 2, ch * 2), (0, 0, 0, 0))
    d = ImageDraw.Draw(hi, "RGBA")
    ox, oy = 4, 4
    pts = [
        (ox, oy), (ox, oy + 32), (ox + 8.4, oy + 25.2),
        (ox + 14.8, oy + 39.0), (ox + 20.4, oy + 36.4),
        (ox + 14.0, oy + 22.8), (ox + 24.4, oy + 22.8),
    ]
    pts_hi = [(p[0] * 2, p[1] * 2) for p in pts]
    d.polygon([(p[0] + 4, p[1] + 6) for p in pts_hi], fill=(0, 0, 0, 120))
    d.polygon(pts_hi, fill=(255, 255, 255, 255), outline=(14, 14, 16, 255), width=4)
    return hi.resize((cw, ch), LANCZOS), (ox, oy)


def make_ball_2x():
    s = 96
    hi = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(hi, "RGBA")
    d.ellipse([6, 6, s - 6, s - 6], fill=(*C_BALL, 255), outline=(42, 24, 24, 255), width=8)
    d.line([(20, 74), (74, 20)], fill=(*C_BALL_STRIPE, 255), width=14)
    d.ellipse([24, 22, 42, 38], fill=(255, 248, 236, 255))
    return hi.resize((22 * S, 22 * S), LANCZOS)


def make_heart_2x():
    art = [".##.##.", "#######", "#######", ".#####.", "..###..", "...#..."]
    sc = 4
    im = Image.new("RGBA", (7 * sc, 6 * sc), (0, 0, 0, 0))
    d = ImageDraw.Draw(im, "RGBA")
    for ry, row in enumerate(art):
        for rx, ch in enumerate(row):
            if ch == "#":
                col = C_HEART_HI if (ry == 1 and rx in (1, 2)) else C_HEART
                d.rectangle([rx * sc, ry * sc, (rx + 1) * sc - 1, (ry + 1) * sc - 1], fill=(*col, 255))
    return im


def make_bubble_2x(text):
    pad_x, pad_y = 13 * S, 7 * S
    dummy = ImageDraw.Draw(Image.new("RGBA", (1, 1)))
    bb = dummy.textbbox((0, 0), text, font=F_BUBBLE)
    tw, th = bb[2] - bb[0], bb[3] - bb[1]
    bw, bh = tw + pad_x * 2, th + pad_y * 2 + 4
    tail_h = 7 * S
    im = Image.new("RGBA", (bw, bh + tail_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(im, "RGBA")
    d.rounded_rectangle([0, 0, bw - 1, bh - 1], radius=8 * S, fill=(*C_BUBBLE_BG, 252), outline=(*C_AMBER, 255), width=4)
    mid = bw // 2
    d.polygon([(mid - 10, bh - 4), (mid + 10, bh - 4), (mid, bh + tail_h - 2)], fill=(*C_BUBBLE_BG, 252))
    d.line([(mid - 10, bh - 2), (mid, bh + tail_h - 2), (mid + 10, bh - 2)], fill=(*C_AMBER, 255), width=4)
    draw_centered(d, bw / 2.0, bh / 2.0, text, F_BUBBLE, (250, 246, 238, 255))
    return im


def draw_chapter_pill(frame_1x, num, title, accent):
    ov = Image.new("RGBA", (OUT_W, OUT_H), (0, 0, 0, 0))
    d = ImageDraw.Draw(ov, "RGBA")
    f1 = get_font(5.5, bold=True, mono=True)
    f2 = get_font(5.5, bold=False)
    tag = f"0{num}"
    tw = int(d.textbbox((0, 0), tag, font=f1)[2] + 12)
    dw = int(d.textbbox((0, 0), title, font=f2)[2] + 14)
    tot = tw + dw + 12
    px = (OUT_W - tot) // 2
    py = 29
    d.rounded_rectangle([px, py, px + tot, py + 22], radius=11, fill=(22, 25, 32, 224), outline=(255, 255, 255, 42), width=1)
    d.rounded_rectangle([px + 4, py + 4, px + 4 + tw, py + 18], radius=7, fill=(*accent, 52))
    draw_centered(d, px + 4 + tw / 2.0, py + 11.0, tag, f1, accent)
    draw_centered(d, px + 8 + tw + dw / 2.0, py + 11.0, title, f2, C_TEXT)
    frame_1x.alpha_composite(ov)


def apply_camera(frame_2x, zoom, focus_x_pt, focus_y_pt):
    if zoom <= 1.001:
        return frame_2x.resize((OUT_W, OUT_H), LANCZOS)
    cw = W / zoom
    ch = H / zoom
    cx = focus_x_pt * S
    cy = focus_y_pt * S
    x0 = clamp(cx - cw / 2.0, 0.0, W - cw)
    y0 = clamp(cy - ch / 2.0, 0.0, H - ch)
    cropped = frame_2x.crop((round(x0), round(y0), round(x0 + cw), round(y0 + ch)))
    return cropped.resize((OUT_W, OUT_H), LANCZOS)


def generate_frames():
    packs = {
        "highland_cow": load_pack_anims("highland_cow", 76),
        "dog": load_pack_anims("dog", 58),
        "polar_bear": load_pack_anims("polar_bear", 60),
        "stegosaurus": load_pack_anims("stegosaurus", 58),
    }
    dock_icons = build_og_dock_icons()
    static_stage = build_static_stage(dock_icons)
    cursor_2x, hot_2x = make_cursor_2x()
    ball_2x = make_ball_2x()
    heart_2x = make_heart_2x()
    bubble_build = make_bubble_2x("BUILD PASSED!")
    bubble_play = make_bubble_2x("PLAY TIME!")

    # Clean lanes (Bear stays left of chapter pill; Dog stays right of chapter pill)
    dock_cow_y = DOCK_TOP - packs["highland_cow"]["gy_pt"] + 1.0
    vsc_cow_y = R_BOX[1] - packs["highland_cow"]["gy_pt"] + 1.0
    cow_w = packs["highland_cow"]["w_pt"]

    cow_x, cow_y, cow_dir = 435.0, dock_cow_y, 1
    bear_x, bear_dir = 84.0, 1
    dog_x, dog_dir = 780.0, -1
    stego_x, stego_dir = 64.0, 1

    ball_active = False
    ball_x, ball_y, ball_vx, ball_vy = 560.0, 44.0, -2.4, 0.0
    ball_kicked = False

    cur = (710.0, 240.0)
    frames_rgb = []

    for f in range(TOTAL_FRAMES):
        fr = static_stage.copy()

        typed = 0
        if 100 <= f < 142:
            typed = min(len(CMD_REACT), int((f - 100) * 1.18))
        elif f >= 142:
            typed = len(CMD_REACT)

        state = {
            "term_focused": 90 <= f < 174,
            "typed_chars": typed,
            "react_done": f >= 142,
            "ball_dropped": f >= 214,
        }
        draw_terminal_window(fr, state)

        # Background Herd Motion (safe bounds so Bear & Dog never clip centered chapter pill!)
        bear_x += bear_dir * 1.4
        if bear_x > 290.0:
            bear_dir = -1
        elif bear_x < L_BOX[0] + 18.0:
            bear_dir = 1

        dog_x += dog_dir * 1.5
        if dog_x > R_BOX[2] - packs["dog"]["w_pt"] - 18.0:
            dog_dir = -1
        elif dog_x < 650.0:
            dog_dir = 1

        stego_x += stego_dir * 1.15
        if stego_x > 215.0:
            stego_dir = -1
        elif stego_x < 34.0:
            stego_dir = 1

        # Scenario 1 (0..84): Drag & Toss Highland Cow onto VS Code Titlebar
        cow_mode = "walk"
        if f < 26:
            cow_x += cow_dir * 1.8
            if cow_x > 540.0:
                cow_dir = -1
            elif cow_x < 345.0:
                cow_dir = 1
            cow_y = dock_cow_y
        elif f < 50:
            t = smootherstep((f - 26) / 24.0)
            cow_mode = "drag"
            cow_x = lerp(472.0, 540.0, t)
            cow_y = lerp(dock_cow_y, vsc_cow_y - 42.0, t)
        elif f < 60:
            t = (f - 50) / 10.0
            cow_mode = "fall"
            cow_x = lerp(540.0, 560.0, t)
            cow_y = lerp(vsc_cow_y - 42.0, vsc_cow_y, t * t)
        elif f < 178:
            cow_x += cow_dir * 1.4
            if cow_x > 640.0:
                cow_dir = -1
            elif cow_x < 500.0:
                cow_dir = 1
            cow_y = vsc_cow_y
        elif f < 190:
            t = (f - 178) / 12.0
            cow_mode = "fall"
            cow_x = lerp(cow_x, 424.0, t)
            cow_y = lerp(vsc_cow_y, dock_cow_y, t * t) - math.sin(t * math.pi) * 26.0
        else:
            cow_y = dock_cow_y

        # Scenario 3 (214..289): Play Mode Bouncy Ball Physics
        if f == 214:
            ball_active = True
            ball_x, ball_y, ball_vx, ball_vy = 584.0, 44.0, -2.2, 0.0
            ball_kicked = False

        if ball_active:
            ball_vy += 0.72
            ball_x += ball_vx
            ball_y += ball_vy
            if ball_x < 28.0:
                ball_x = 28.0
                ball_vx = abs(ball_vx) * 0.85
            elif ball_x > OUT_W - 50.0:
                ball_x = OUT_W - 50.0
                ball_vx = -abs(ball_vx) * 0.85
            b_floor = DOCK_TOP - 22.0 if (DOCK_BOX[0] - 10 <= ball_x <= DOCK_BOX[2] + 10) else FLOOR_Y - 22.0
            if ball_y >= b_floor:
                ball_y = b_floor
                ball_vy = -ball_vy * 0.74 if abs(ball_vy) > 2.2 else 0.0
                ball_vx *= 0.96

            cow_cx = cow_x + cow_w * 0.5
            dx_b = (ball_x + 11.0) - cow_cx
            if not ball_kicked and abs(dx_b) < 28.0 and abs((ball_y + 11.0) - (DOCK_TOP - 20.0)) < 48.0:
                ball_kicked = True
                ball_vy = -13.6
                ball_vx = 7.2 if dx_b >= 0 else -7.2
            elif abs(dx_b) > 16.0:
                cow_dir = 1 if dx_b > 0 else -1
                cow_x = clamp(cow_x + cow_dir * 3.4, DOCK_BOX[0] + 6.0, DOCK_BOX[2] - cow_w - 6.0)

        herd_hop = 0.0
        if 142 <= f <= 158:
            herd_hop = -math.sin(((f - 142) / 16.0) * math.pi) * 22.0

        fi_fast = (f // 4) % 4
        fi_med = (f // 5) % 4
        fi_slow = (f // 6) % 4

        def blit_pet(pid, x_pt, y_pt, direction, mode="walk", fi=0):
            pk = packs[pid]
            frames_list = pk[f"{mode}_{'r' if direction > 0 else 'l'}"]
            fr.alpha_composite(frames_list[fi % len(frames_list)], (round(x_pt * S), round(y_pt * S)))

        blit_pet("polar_bear", bear_x, L_BOX[1] - packs["polar_bear"]["gy_pt"] + 1.0 + herd_hop, bear_dir, "walk", fi_med)
        blit_pet("dog", dog_x, R_BOX[1] - packs["dog"]["gy_pt"] + 1.0 + herd_hop, dog_dir, "walk", fi_med)
        blit_pet("stegosaurus", stego_x, FLOOR_Y - packs["stegosaurus"]["gy_pt"] + 1.0 + herd_hop, stego_dir, "walk", fi_slow)
        blit_pet("highland_cow", cow_x, cow_y + herd_hop, cow_dir, cow_mode, fi_fast)

        # Floating Pixel Hearts (framed to left/right of cow so they NEVER sit behind speech bubble tail!)
        show_react = 142 <= f <= 178
        show_kick = ball_kicked and f < 276
        if show_react or show_kick:
            t0 = 142 if show_react else 244
            p = ((f - t0) % 24) / 24.0
            targets = [(cow_x + cow_w * 0.5, cow_y + herd_hop, 32.0)]
            if show_react:
                targets.append((bear_x + packs["polar_bear"]["w_pt"] * 0.5, L_BOX[1] - packs["polar_bear"]["gy_pt"] + herd_hop, 16.0))
                targets.append((dog_x + packs["dog"]["w_pt"] * 0.5, R_BOX[1] - packs["dog"]["gy_pt"] + herd_hop, 16.0))
            for tx, ty, spread in targets:
                for k, (ox, ph) in enumerate([(-spread, 0.0), (spread - 6.0, 0.40)]):
                    pp = (p + ph) % 1.0
                    hx = round((tx + ox + math.sin(pp * math.tau + k) * 3.5) * S)
                    hy = round((ty + 18.0 - pp * 24.0) * S)
                    if 0 <= hx < W - 28 and MENUBAR_H * S <= hy < H - 24:
                        fr.alpha_composite(heart_2x, (hx, hy))

        # Floating Amber Speech Bubbles (strictly below VS Code's 378pt bottom when on Dock!)
        active_bub = bubble_build if (142 <= f <= 178) else (bubble_play if (ball_kicked and f < 276) else None)
        if active_bub is not None:
            bx = round(clamp((cow_x + cow_w * 0.5) * S - active_bub.width * 0.5, 24.0, W - active_bub.width - 24.0))
            raw_by = round((cow_y + herd_hop + 6.0) * S - active_bub.height)
            min_by = (R_BOX[3] + 6) * S if (f >= 190) else (MENUBAR_H + 4) * S
            by = max(min_by, raw_by)
            fr.alpha_composite(active_bub, (bx, by))

        if ball_active:
            fr.alpha_composite(ball_2x, (round(ball_x * S), round(ball_y * S)))

        # Cursor & Native macOS Context Menu
        rip = None
        menu_target = (cow_x + cow_w * 0.5 + 8.0, cow_y + 26.0)
        if f < 14:
            cur = (710.0, 240.0)
        elif f < 26:
            t = smootherstep((f - 14) / 12.0)
            cur = (lerp(710.0, cow_x + cow_w * 0.5, t), lerp(240.0, cow_y + 28.0, t))
        elif f < 50:
            cur = (cow_x + cow_w * 0.5, cow_y + 24.0)
        elif f < 88:
            t = smootherstep((f - 50) / 22.0)
            cur = (lerp(540.0 + cow_w * 0.5, 250.0, t), lerp(vsc_cow_y - 18.0, 300.0, t))
        elif f < 182:
            cur = (250.0, 300.0)
        elif f < 194:
            t = smootherstep((f - 182) / 12.0)
            cur = (lerp(250.0, menu_target[0], t), lerp(300.0, menu_target[1], t))
        elif f < 216:
            hov = 1 if f >= 204 else -1
            btn_x, btn_y = draw_macos_context_menu(fr, menu_target[0] + 6.0, menu_target[1] - 4.0, hov, pressed=(211 <= f <= 215))
            if f < 204:
                t = smootherstep((f - 196) / 8.0)
                cur = (lerp(menu_target[0], btn_x, t), lerp(menu_target[1], btn_y, t))
            else:
                cur = (btn_x, btn_y)
            if 212 <= f < 216:
                rip = (btn_x, btn_y, (f - 212) / 10.0, C_ACCENT_BLUE)
        else:
            if 216 <= f < 224:
                rip = (cur[0], cur[1], (f - 212) / 10.0, C_ACCENT_BLUE)

        if rip is not None:
            rx, ry, rp, rcol = rip
            e = ease_out_cubic(rp)
            rr = (4 + 22 * e) * S
            rov = Image.new("RGBA", (W, H), (0, 0, 0, 0))
            ImageDraw.Draw(rov, "RGBA").ellipse(
                [rx * S - rr, ry * S - rr, rx * S + rr, ry * S + rr],
                outline=(*rcol, int(245 * (1.0 - e))),
                width=4,
            )
            fr.alpha_composite(rov)

        fr.alpha_composite(cursor_2x, (round(cur[0] * S - hot_2x[0]), round(cur[1] * S - hot_2x[1])))

        # Smooth Virtual Camera Zoom
        cam_zoom, cam_x, cam_y = 1.0, 480.0, 270.0
        if 86 <= f < 102:
            t = smootherstep((f - 86) / 16.0)
            cam_zoom, cam_x, cam_y = lerp(1.0, 1.38, t), lerp(480.0, 278.0, t), lerp(270.0, 260.0, t)
        elif 102 <= f < 142:
            cam_zoom, cam_x, cam_y = 1.38, 278.0, 260.0
        elif 142 <= f < 156:
            t = smootherstep((f - 142) / 14.0)
            cam_zoom, cam_x, cam_y = lerp(1.38, 1.0, t), lerp(278.0, 480.0, t), lerp(260.0, 270.0, t)
        elif 192 <= f < 204:
            t = smootherstep((f - 192) / 12.0)
            cam_zoom, cam_x, cam_y = lerp(1.0, 1.22, t), lerp(480.0, 500.0, t), lerp(270.0, 395.0, t)
        elif 204 <= f < 216:
            cam_zoom, cam_x, cam_y = 1.22, 500.0, 395.0
        elif 216 <= f < 230:
            t = smootherstep((f - 216) / 14.0)
            cam_zoom, cam_x, cam_y = lerp(1.22, 1.0, t), lerp(500.0, 480.0, t), lerp(395.0, 270.0, t)

        out_1x = apply_camera(fr, cam_zoom, cam_x, cam_y)
        if f < 86:
            draw_chapter_pill(out_1x, 1, "Drag, Toss & Walk on Any Window", C_CYAN)
        elif f < 184:
            draw_chapter_pill(out_1x, 2, "AI Agent MCP & Speech Bubbles", C_GREEN)
        else:
            draw_chapter_pill(out_1x, 3, "Play Mode · Bouncy Ball & Herd Chase", C_AMBER)

        frames_rgb.append(out_1x.convert("RGB"))

    fade_len = 12
    first_rgb = frames_rgb[0]
    for i in range(fade_len):
        idx = len(frames_rgb) - fade_len + i
        frames_rgb[idx] = Image.blend(frames_rgb[idx], first_rgb, ease_out_cubic((i + 1) / float(fade_len)))

    return frames_rgb, packs


def extract_top_sprite_colors(packs):
    """Harvest actual dominant RGB colors from all 4 real mascot sprites so ZERO pet colors turn grey!"""
    colors = []
    for pk in packs.values():
        im = pk["sample_tile"].convert("RGBA")
        counts = {}
        for r, g, b, a in im.getdata():
            if a < 200:
                continue
            bucket = (r // 16 * 16 + 8, g // 16 * 16 + 8, b // 16 * 16 + 8)
            counts[bucket] = counts.get(bucket, 0) + 1
        top = sorted(counts.items(), key=lambda kv: -kv[1])[:8]
        colors.extend([c for c, _ in top])
    return colors


def save_outputs(frames_rgb, packs):
    os.makedirs(os.path.dirname(OUT_GIF), exist_ok=True)
    n = len(frames_rgb)

    sample_indices = [0, 36, 65, 118, 152, 206, 232, 260]
    ui_pinned = [
        C_BG_EDGE, C_BG_GLOW, C_MENUBAR, C_TERM_BG, C_TERM_BAR, C_VSC_CHROME, C_VSC_EDITOR,
        C_VSC_SEL, C_VSC_STATUS, C_TL_RED, C_TL_YELLOW, C_TL_GREEN, C_ACCENT_BLUE,
        C_TEXT, C_MUTED, C_GREEN, C_CYAN, C_KW, C_FN, C_TYPE, C_STR, C_CM,
        C_AMBER, C_BUBBLE_BG, C_HEART, C_HEART_HI, C_BALL, C_BALL_STRIPE,
        (36, 158, 255), (238, 248, 255), (0, 122, 204), (31, 156, 240), (255, 59, 48), (108, 196, 252),
    ]
    sprite_pinned = extract_top_sprite_colors(packs)
    pinned = ui_pinned + sprite_pinned

    swatch_h = 96
    strip = Image.new("RGB", (OUT_W, OUT_H * len(sample_indices) + swatch_h))
    for i, idx in enumerate(sample_indices):
        strip.paste(frames_rgb[min(idx, n - 1)], (0, i * OUT_H))

    sd = ImageDraw.Draw(strip)
    sy = OUT_H * len(sample_indices)
    # 2 swatch rows so every UI + Mascot color gets generous area weight in MEDIANCUT!
    half = (len(pinned) + 1) // 2
    for row_i, row_cols in enumerate([pinned[:half], pinned[half:]]):
        if not row_cols:
            continue
        sw = max(1, OUT_W // len(row_cols))
        y0 = sy + row_i * (swatch_h // 2)
        y1 = y0 + (swatch_h // 2)
        for i, col in enumerate(row_cols):
            sd.rectangle([i * sw, y0, (i + 1) * sw, y1], fill=col)

    master = strip.quantize(colors=256, method=MEDIANCUT, dither=DITHER_NONE)
    paletted = [fr.quantize(palette=master, dither=DITHER_NONE) for fr in frames_rgb]
    paletted[0].save(
        OUT_GIF,
        save_all=True,
        append_images=paletted[1:],
        duration=1000 // FPS,
        loop=0,
        optimize=True,
        disposal=1,
    )
    print(f"Saved OG-Icon 2x-Retina Launch GIF: {OUT_GIF} ({OUT_W}x{OUT_H}, {n} frames @ {FPS} FPS, {os.path.getsize(OUT_GIF)/1024:.1f} KB)")

    ffmpeg = shutil.which("ffmpeg")
    if ffmpeg:
        with tempfile.TemporaryDirectory() as td:
            for i, fr in enumerate(frames_rgb):
                fr.save(os.path.join(td, f"f_{i:04d}.png"), "PNG")
            cmd = [
                ffmpeg, "-y", "-framerate", str(FPS),
                "-i", os.path.join(td, "f_%04d.png"),
                "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18", "-movflags", "+faststart",
                OUT_MP4,
            ]
            subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            print(f"Saved 24-bit MP4: {OUT_MP4} ({os.path.getsize(OUT_MP4)/1024:.1f} KB)")


if __name__ == "__main__":
    frames, loaded_packs = generate_frames()
    save_outputs(frames, loaded_packs)