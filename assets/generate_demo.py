import base64
import io
import math
import os
import re
import shutil
import subprocess
import tempfile
from PIL import Image, ImageDraw, ImageFont

# --- Output Spec (960x540 16:9, exact 25 FPS = 40ms GIF delay) ---
OUT_W, OUT_H = 960, 540
S = 2  # 2x Retina vector chrome & crisp zoom; sprites stay razor-sharp
W, H = OUT_W * S, OUT_H * S
FPS = 25
TOTAL_FRAMES = 280  # 11.2s across 3 clean scenarios

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
CRATE_DIR = os.path.dirname(SCRIPT_DIR)
OUT_GIF = os.path.join(CRATE_DIR, "brand", "demo.gif")
OUT_MP4 = os.path.join(CRATE_DIR, "brand", "demo.mp4")

LANCZOS = getattr(Image, "Resampling", Image).LANCZOS
NEAREST = getattr(Image, "Resampling", Image).NEAREST
MEDIANCUT = getattr(Image, "Quantize", Image).MEDIANCUT
DITHER_NONE = getattr(Image, "Dither", Image).NONE

# --- Verified macOS Dark Mode Design Tokens (Flat Matte -> Zero Palette Starvation) ---
C_DESKTOP = (18, 20, 24)        # #121418 clean flat matte backdrop
C_SHADOW_1 = (11, 13, 16)       # Crisp 2-step outer shadow (zero blur gradient!)
C_SHADOW_2 = (7, 8, 11)         # Crisp 2-step inner shadow
C_MENUBAR = (28, 28, 30)        # #1C1C1E macOS dark menu bar (24pt)
C_MENUBAR_LINE = (48, 48, 52)   # #303034 hairline

# Verified macOS Window & Terminal Dark Tokens
C_WIN_BG = (30, 30, 30)         # #1E1E1E NSColor.windowBackgroundColor
C_WIN_BAR = (45, 45, 47)        # #2D2D2F macOS 28pt titlebar
C_WIN_BAR_DIM = (38, 38, 41)    # #262629 unfocused titlebar
C_WIN_BORDER = (68, 68, 72)     # #444448 window hairline
C_WIN_DIV = (54, 54, 58)        # #36363A subtle divider

# Verified Apple Notes Dark Mode Tokens
C_NOTES_SIDE = (37, 37, 39)     # #252527 Notes dark sidebar
C_NOTES_SEL = (66, 58, 32)      # Subtle warm dark selection row
C_SYS_YELLOW = (255, 214, 10)   # #FFD60A Apple HIG dark mode systemYellow
C_SYS_ORANGE = (255, 159, 10)   # #FF9F0A Apple HIG dark mode systemOrange
C_SYS_BLUE = (10, 132, 255)     # #0A84FF Apple HIG dark mode systemBlue
C_SYS_RED = (255, 69, 58)       # #FF453A Apple HIG dark mode systemRed

# Verified Apple HIG Traffic Lights (12pt diameter, 8pt gap -> 20pt pitch)
C_TL_RED = (255, 95, 87)        # #FF5F57
C_TL_YELLOW = (255, 189, 46)    # #FFBD2E
C_TL_GREEN = (40, 200, 64)      # #28C840
C_TL_OFF = (78, 78, 82)

# Dock & Typography
C_DOCK_BG = (36, 38, 44)        # #24262C clean solid dark-glass Dock (no banding!)
C_DOCK_BORDER = (74, 78, 88)    # #4A4E58 crisp rim
C_TEXT = (235, 239, 245)        # #EBEFF5
C_MUTED = (146, 152, 164)       # #9298A4
C_GREEN = (63, 185, 80)         # #3FB950 terminal success green
C_CYAN = (121, 192, 255)        # #79C0FF terminal prompt cyan
C_AMBER = (242, 138, 56)        # #F28A38 DIYPets warm amber
C_BUBBLE_BG = (22, 26, 36)      # #161A24 speech bubble fill
C_HEART = (240, 60, 98)         # #F03C62 pixel heart
C_HEART_HI = (255, 198, 212)    # #FFC6D4 heart highlight
C_BALL = (242, 104, 62)         # #F2683E toy ball
C_BALL_STRIPE = (255, 224, 128) # #FFE080 ball stripe

# --- Exact Logical Layout (1x pt) ---
MENUBAR_H = 24
TITLEBAR_H = 28
WIN_RADIUS = 10
L_BOX = (24, 78, 472, 376)      # 448 x 298pt macOS Terminal
R_BOX = (488, 78, 936, 376)     # 448 x 298pt Apple Notes (Dark Mode)
DOCK_BOX = (344, 484, 616, 534) # 272 x 50pt clean floating Dock (6pt above bottom)
DOCK_TOP = DOCK_BOX[1]          # 484pt
FLOOR_Y = 534

PROMPT = "sm@mac diypets % "
CMD_REACT = 'diypets react --mood happy --text "Build passed!"'


def get_font(pt, bold=False, mono=False):
    px = int(round(pt * S))
    if mono:
        cands = (
            ["C:\\Windows\\Fonts\\consolab.ttf", "DejaVuSansMono-Bold.ttf"]
            if bold else
            ["C:\\Windows\\Fonts\\consola.ttf", "DejaVuSansMono.ttf"]
        )
    else:
        cands = (
            ["C:\\Windows\\Fonts\\segoeuib.ttf", "arialbd.ttf", "DejaVuSans-Bold.ttf"]
            if bold else
            ["C:\\Windows\\Fonts\\segoeui.ttf", "arial.ttf", "DejaVuSans.ttf"]
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
F_NOTE_H = get_font(14.5, bold=True)
F_SM = get_font(10.5)
F_SM_B = get_font(10.5, bold=True)
F_XS = get_font(9.5)
F_XS_B = get_font(9.5, bold=True)
F_MONO = get_font(9.6, mono=True)
F_MONO_B = get_font(9.6, bold=True, mono=True)
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
    """Load real walk, drag, and fall frames from assets/<pack>/deskpet.xml.
    Stores both 1x crisp NEAREST frames (for razor-sharp non-zoomed output) and 2x frames (for zoom)."""
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

    # Exact clean size in 1x logical pixels; 2x version is strict 2x pixel-doubled (zero blur!)
    scale_1x = target_h_pt / float(th)
    w1 = max(1, round(tw * scale_1x))
    h1 = max(1, round(th * scale_1x))
    gy_pt = gy * scale_1x

    def build_set(ids):
        l1, r1, l2, r2 = [], [], [], []
        for idx in ids:
            cx, cy = (idx % tx) * tw, (idx // tx) * th
            tile = sheet.crop((cx, cy, cx + tw, cy + th))
            # Crisp 1x sprite using NEAREST (zero anti-aliased edge greying!)
            sp1 = tile.resize((w1, h1), NEAREST)
            # Clean any semi-alpha pixels cleanly
            px = sp1.load()
            for y in range(h1):
                for x in range(w1):
                    r, g, b, a = px[x, y]
                    if a < 128:
                        px[x, y] = (0, 0, 0, 0)
                    elif a < 255:
                        px[x, y] = (r, g, b, 255)
            sp1_r = sp1.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
            l1.append(sp1)
            r1.append(sp1_r)
            l2.append(sp1.resize((w1 * S, h1 * S), NEAREST))
            r2.append(sp1_r.resize((w1 * S, h1 * S), NEAREST))
        return l1, r1, l2, r2

    wl1, wr1, wl2, wr2 = build_set(walk_ids)
    dl1, dr1, dl2, dr2 = build_set(drag_ids)
    fl1, fr1, fl2, fr2 = build_set(fall_ids)
    return {
        "walk_l_1x": wl1, "walk_r_1x": wr1, "walk_l_2x": wl2, "walk_r_2x": wr2,
        "drag_l_1x": dl1, "drag_r_1x": dr1, "drag_l_2x": dl2, "drag_r_2x": dr2,
        "fall_l_1x": fl1, "fall_r_1x": fr1, "fall_l_2x": fl2, "fall_r_2x": fr2,
        "w_pt": float(w1), "h_pt": float(h1), "gy_pt": gy_pt,
        "sample_tile": wl1[0],
    }


def draw_traffic_lights(d, x0_pt, cy_pt, active=True):
    cols = (C_TL_RED, C_TL_YELLOW, C_TL_GREEN) if active else (C_TL_OFF, C_TL_OFF, C_TL_OFF)
    r = 6 * S
    for i, col in enumerate(cols):
        cx = (x0_pt + i * 20) * S
        cy = cy_pt * S
        d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(*col, 255))


# --- Authentic Default macOS Cursor (BLACK fill #000000 + crisp WHITE outline #FFFFFF) ---
def make_macos_cursor(scale_out):
    """4x supersampled authentic macOS default arrow cursor (black interior, white outline)."""
    hi_w, hi_h = 88, 104
    hi = Image.new("RGBA", (hi_w, hi_h), (0, 0, 0, 0))
    d = ImageDraw.Draw(hi, "RGBA")
    # Classic macOS pointer geometry (clean sharp 17pt tall arrow, slight angled stem)
    ox, oy = 8.0, 8.0
    pts = [
        (ox, oy),
        (ox, oy + 62.0),
        (ox + 15.0, oy + 48.0),
        (ox + 26.5, oy + 72.5),
        (ox + 36.0, oy + 68.0),
        (ox + 24.5, oy + 44.0),
        (ox + 44.0, oy + 44.0),
    ]
    # Subtle drop shadow
    d.polygon([(p[0] + 4, p[1] + 6) for p in pts], fill=(0, 0, 0, 95))
    # Crisp white outer stroke + pure black macOS fill
    d.polygon(pts, fill=(0, 0, 0, 255), outline=(255, 255, 255, 255), width=7)
    out_w = int(round(22 * scale_out))
    out_h = int(round(26 * scale_out))
    return hi.resize((out_w, out_h), LANCZOS), (int(round(2 * scale_out)), int(round(2 * scale_out)))


# --- Crisp, Easy, Unmistakable macOS Squircle Dock Icons (22.5% radius) ---
def make_squircle_icon(out_px, render_hi_fn):
    hi_s = 256
    rad = round(hi_s * 0.225)
    canvas = Image.new("RGBA", (hi_s, hi_s), (0, 0, 0, 0))
    render_hi_fn(canvas, hi_s)
    mask = Image.new("L", (hi_s, hi_s), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, hi_s - 1, hi_s - 1], radius=rad, fill=255)
    clipped = Image.new("RGBA", (hi_s, hi_s), (0, 0, 0, 0))
    clipped.paste(canvas, (0, 0), mask)
    return clipped.resize((out_px, out_px), LANCZOS)


def build_easy_dock_icons():
    sz = 36 * S  # 72px Retina squircles

    # 1. macOS Terminal (metallic bezel + #1E1E1E screen + crisp >_)
    def r_terminal(im, s):
        d = ImageDraw.Draw(im, "RGBA")
        d.rectangle([0, 0, s, s], fill=(82, 86, 94, 255))
        d.rounded_rectangle([18, 18, s - 18, s - 18], radius=36, fill=(24, 25, 29, 255))
        d.line([(54, 74), (96, 108), (54, 142)], fill=(240, 244, 250, 255), width=16, joint="curve")
        d.line([(108, 146), (160, 146)], fill=(240, 244, 250, 255), width=16)

    # 2. Apple Notes (iconic #FFD60A yellow header + clean white ruled notepad)
    def r_notes(im, s):
        d = ImageDraw.Draw(im, "RGBA")
        d.rectangle([0, 0, s, s], fill=(246, 246, 248, 255))
        d.rectangle([0, 0, s, 72], fill=(*C_SYS_YELLOW, 255))
        d.line([(0, 74), (s, 74)], fill=(214, 214, 218, 255), width=4)
        for ry in (118, 162, 206):
            d.line([(24, ry), (s - 24, ry)], fill=(212, 214, 220, 255), width=5)

    # 3. Apple Calendar (iconic #FF453A red header "MON" + clean bold "23")
    def r_calendar(im, s):
        d = ImageDraw.Draw(im, "RGBA")
        d.rectangle([0, 0, s, s], fill=(248, 248, 250, 255))
        d.rectangle([0, 0, s, 76], fill=(*C_SYS_RED, 255))
        f_mon = get_font(22, bold=True)
        f_day = get_font(62, bold=True)
        draw_centered(d, s / 2, 40, "MON", f_mon, (255, 255, 255, 255))
        draw_centered(d, s / 2, 166, "23", f_day, (28, 30, 34, 255))

    # 4. DIYPets Highland Cow App Icon (warm cream squircle + crisp real cow)
    def r_diypets(im, s):
        d = ImageDraw.Draw(im, "RGBA")
        d.rectangle([0, 0, s, s], fill=(252, 240, 222, 255))
        cow_p = os.path.join(CRATE_DIR, "brand", "logo-512.png")
        if os.path.isfile(cow_p):
            ci = Image.open(cow_p).convert("RGBA").resize((s - 32, s - 32), LANCZOS)
            im.alpha_composite(ci, (16, 16))

    # 5. Clean macOS Blue Folder
    def r_folder(im, s):
        d = ImageDraw.Draw(im, "RGBA")
        d.rectangle([0, 0, s, s], fill=(34, 40, 52, 255))
        d.rounded_rectangle([34, 62, 122, 108], radius=14, fill=(58, 152, 232, 255))
        d.rounded_rectangle([34, 78, s - 34, s - 46], radius=16, fill=(64, 160, 238, 255))
        d.rounded_rectangle([34, 96, s - 34, s - 42], radius=16, fill=(108, 196, 252, 255))

    return [
        (make_squircle_icon(sz, r_terminal), True),
        (make_squircle_icon(sz, r_notes), True),
        (make_squircle_icon(sz, r_calendar), False),
        (make_squircle_icon(sz, r_diypets), True),
        None,
        (make_squircle_icon(sz, r_folder), False),
    ]


def build_static_stage(dock_icons):
    """Flat matte #121418 macOS desktop + crisp 2-step shadows + Apple Notes + floating Dock."""
    bg = Image.new("RGBA", (W, H), (*C_DESKTOP, 255))
    d = ImageDraw.Draw(bg, "RGBA")

    # 1. Clean macOS Menu Bar (exact 24pt = 48px — clean typography, zero broken Apple hack!)
    d.rectangle([0, 0, W, MENUBAR_H * S], fill=(*C_MENUBAR, 255))
    d.line([(0, MENUBAR_H * S - 1), (W, MENUBAR_H * S - 1)], fill=(*C_MENUBAR_LINE, 255), width=2)
    d.text((18 * S, 5 * S), "DIYPets", font=F_MENU_B, fill=C_TEXT)
    d.text((78 * S, 5 * S), "Herd", font=F_MENU, fill=C_MUTED)
    d.text((118 * S, 5 * S), "Studio", font=F_MENU, fill=C_MUTED)
    d.text((166 * S, 5 * S), "Play", font=F_MENU, fill=C_MUTED)
    d.text((204 * S, 5 * S), "Window", font=F_MENU, fill=C_MUTED)

    # Right menu bar status (Highland Cow tray icon + clean clock)
    cow_p = os.path.join(CRATE_DIR, "brand", "logo-512.png")
    if os.path.isfile(cow_p):
        tray_cow = Image.open(cow_p).convert("RGBA").resize((15 * S, 15 * S), LANCZOS)
        bg.alpha_composite(tray_cow, (852 * S, 4 * S))
    d.text((878 * S, 5 * S), "Mon 10:42", font=F_MENU, fill=C_TEXT)

    # 2. Crisp 2-Step Drop Shadows for Both Windows (zero Gaussian gradient -> preserves 256-color palette!)
    for x0, y0, x1, y1 in (L_BOX, R_BOX):
        d.rounded_rectangle([x0 * S - 4, y0 * S + 8, x1 * S + 4, y1 * S + 16], radius=WIN_RADIUS * S + 2, fill=(*C_SHADOW_1, 255))
        d.rounded_rectangle([x0 * S - 2, y0 * S + 4, x1 * S + 2, y1 * S + 10], radius=WIN_RADIUS * S + 1, fill=(*C_SHADOW_2, 255))

    # 3. Right Window: Authentic Apple Notes (Dark Mode) — Easy, Clean, Unmistakable!
    rx0, ry0, rx1, ry1 = [v * S for v in R_BOX]
    rad = WIN_RADIUS * S
    th = TITLEBAR_H * S
    sb_w = 138 * S

    d.rounded_rectangle([rx0, ry0, rx1, ry1], radius=rad, fill=(*C_WIN_BG, 255), outline=(*C_WIN_BORDER, 255), width=2)
    # Left sidebar (#252527)
    d.rounded_rectangle([rx0 + 2, ry0 + 2, rx0 + sb_w, ry1 - 2], radius=rad - 2, fill=(*C_NOTES_SIDE, 255))
    d.rectangle([rx0 + sb_w - rad, ry0 + 2, rx0 + sb_w, ry1 - 2], fill=(*C_NOTES_SIDE, 255))
    # Unified macOS Titlebar
    d.rounded_rectangle([rx0 + 2, ry0 + 2, rx1 - 2, ry0 + th], radius=rad - 2, fill=(*C_WIN_BAR, 255))
    d.rectangle([rx0 + 2, ry0 + th - rad, rx1 - 2, ry0 + th], fill=(*C_WIN_BAR, 255))
    d.line([(rx0 + 2, ry0 + th), (rx1 - 2, ry0 + th)], fill=(*C_WIN_DIV, 255), width=2)
    d.line([(rx0 + sb_w, ry0 + th), (rx0 + sb_w, ry1 - 2)], fill=(*C_WIN_DIV, 255), width=2)

    draw_traffic_lights(d, R_BOX[0] + 18, R_BOX[1] + 14, active=True)
    draw_centered(d, (rx0 + sb_w + rx1) / 2, ry0 + th / 2, "Notes — DIYPets Launch", F_TITLE, C_TEXT)

    # Notes Sidebar items (clean rounded active card in warm dark tint)
    notes_list = [
        ("DIYPets v0.1.6", "10:42 AM · Ready to ship", True),
        ("MCP Tools", "Yesterday · 6 endpoints", False),
        ("Pet Roster", "Mon · 5 SM + 27 eSheep", False),
        ("Studio V3 Ideas", "Sun · SVG auto-rig", False),
    ]
    for i, (title, sub, is_sel) in enumerate(notes_list):
        ny = ry0 + th + 16 + i * 116
        if is_sel:
            d.rounded_rectangle([rx0 + 14, ny - 6, rx0 + sb_w - 14, ny + 92], radius=14, fill=(*C_NOTES_SEL, 255))
        else:
            d.line([(rx0 + 24, ny + 96), (rx0 + sb_w - 20, ny + 96)], fill=(*C_WIN_DIV, 255), width=2)
        d.text((rx0 + 28, ny + 6), title, font=F_SM_B, fill=C_TEXT)
        d.text((rx0 + 28, ny + 44), sub, font=F_XS, fill=C_MUTED)

    # Notes Main Content Area (crisp official #FFD60A yellow accents & check circles!)
    nx = rx0 + sb_w + 34
    ny = ry0 + th + 24
    d.text((nx, ny), "DIYPets v0.1.6 Launch", font=F_NOTE_H, fill=C_TEXT)
    d.text((nx, ny + 44), "November 23, 2026 at 10:42 AM", font=F_XS, fill=C_MUTED)

    checklist = [
        ("Pure Rust engine (~7.8 MB RAM)", True),
        ("Drag, toss & walk on any window", True),
        ("Multi-Pet Herd (per-pet sizes)", True),
        ("MCP & CLI speech bubbles (:3721)", True),
        ("Play Mode bouncy ball & hearts", True),
        ("Built-in 2D / SVG Studio V3", True),
    ]
    cy = ny + 92
    for label, checked in checklist:
        cr = 12
        ccx, ccy = nx + cr, cy + 14
        if checked:
            d.ellipse([ccx - cr, ccy - cr, ccx + cr, ccy + cr], fill=(*C_SYS_YELLOW, 255))
            d.line([(ccx - 6, ccy + 1), (ccx - 1, ccy + 6), (ccx + 7, ccy - 5)], fill=(24, 24, 24, 255), width=4, joint="curve")
        else:
            d.ellipse([ccx - cr, ccy - cr, ccx + cr, ccy + cr], outline=(*C_MUTED, 255), width=3)
        d.text((nx + 38, cy), label, font=F_UI, fill=C_TEXT)
        cy += 58

    # 4. Crisp Floating macOS Dock (solid clean #24262C glass + crisp rim -> zero palette waste!)
    dx0, dy0, dx1, dy1 = [v * S for v in DOCK_BOX]
    d_rad = 16 * S
    d.rounded_rectangle([dx0 - 2, dy0 + 6, dx1 + 2, dy1 + 10], radius=d_rad, fill=(*C_SHADOW_2, 255))
    d.rounded_rectangle([dx0, dy0, dx1, dy1], radius=d_rad, fill=(*C_DOCK_BG, 255), outline=(*C_DOCK_BORDER, 255), width=2)

    ix = dx0 + 22
    iy = dy0 + 12
    for entry in dock_icons:
        if entry is None:
            d.line([(ix + 4, dy0 + 18), (ix + 4, dy1 - 18)], fill=(*C_DOCK_BORDER, 255), width=2)
            ix += 20
            continue
        icon_im, is_running = entry
        bg.alpha_composite(icon_im, (ix, iy))
        if is_running:
            dot_x = ix + icon_im.width // 2
            dot_y = dy1 - 9
            d.ellipse([dot_x - 3, dot_y - 3, dot_x + 3, dot_y + 3], fill=(*C_TEXT, 255))
        ix += icon_im.width + 18

    return bg


def draw_terminal_window(frame, state):
    lx0, ly0, lx1, ly1 = [v * S for v in L_BOX]
    rad = WIN_RADIUS * S
    th = TITLEBAR_H * S
    focused = state["term_focused"]

    d = ImageDraw.Draw(frame, "RGBA")
    d.rounded_rectangle([lx0, ly0, lx1, ly1], radius=rad, fill=(*C_WIN_BG, 255), outline=(*C_WIN_BORDER, 255), width=2)
    tbar = C_WIN_BAR if focused else C_WIN_BAR_DIM
    d.rounded_rectangle([lx0 + 2, ly0 + 2, lx1 - 2, ly0 + th], radius=rad - 2, fill=(*tbar, 255))
    d.rectangle([lx0 + 2, ly0 + th - rad, lx1 - 2, ly0 + th], fill=(*tbar, 255))
    d.line([(lx0 + 2, ly0 + th), (lx1 - 2, ly0 + th)], fill=(*C_WIN_DIV, 255), width=2)
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
    d.text((cx, y), "[mcp]  http://127.0.0.1:3721/mcp (6 tools)", font=F_MONO, fill=C_MUTED)
    y += 34
    d.text((cx, y), "[herd] 4 active: cow · dog · bear · stego", font=F_MONO, fill=C_MUTED)
    y += 48

    shown = CMD_REACT[:state["typed_chars"]]
    d.text((cx, y), PROMPT, font=F_MONO, fill=C_CYAN)
    d.text((cx + pw, y), shown, font=F_MONO, fill=C_TEXT)
    if focused and not state["react_done"]:
        tw = d.textbbox((0, 0), shown, font=F_MONO)[2] if shown else 0
        d.rectangle([cx + pw + tw + 2, y + 2, cx + pw + tw + 12, y + 22], fill=(*C_TEXT, 255))
    y += 40

    if state["react_done"]:
        d.text((cx, y), 'OK  DIYPets reacted: happy ("Build passed!")', font=F_MONO_B, fill=C_GREEN)
        y += 38

    if state["ball_dropped"]:
        d.text((cx, y), '[play] bouncy ball dropped · herd chasing!', font=F_MONO_B, fill=C_SYS_ORANGE)


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

    d = ImageDraw.Draw(frame, "RGBA")
    d.rounded_rectangle([x0 - 2, y0 + 6, x0 + mw + 2, y0 + mh + 10], radius=16, fill=(*C_SHADOW_2, 255))
    d.rounded_rectangle([x0, y0, x0 + mw, y0 + mh], radius=16, fill=(38, 41, 48, 255), outline=(78, 82, 92, 255), width=2)

    cy = y0 + pad
    for idx, lbl in enumerate(items):
        if lbl == "---":
            d.line([(x0 + 18, cy + sep_h // 2), (x0 + mw - 18, cy + sep_h // 2)], fill=(66, 70, 78, 255), width=2)
            cy += sep_h
            continue
        if idx == hovered_idx:
            col = (52, 158, 255, 255) if pressed else (*C_SYS_BLUE, 255)
            d.rounded_rectangle([x0 + 10, cy + 2, x0 + mw - 10, cy + row_h - 2], radius=9, fill=col)
            d.text((x0 + 24, cy + 8), lbl, font=F_UI_B, fill=(255, 255, 255, 255))
        else:
            d.text((x0 + 24, cy + 8), lbl, font=F_UI, fill=C_TEXT)
        cy += row_h
    return (x0 + mw * 0.5) / S, (y0 + pad + row_h * 1.5) / S


def make_ball(scale_out):
    s = 96
    hi = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(hi, "RGBA")
    d.ellipse([6, 6, s - 6, s - 6], fill=(*C_BALL, 255), outline=(42, 24, 24, 255), width=8)
    d.line([(20, 74), (74, 20)], fill=(*C_BALL_STRIPE, 255), width=14)
    d.ellipse([24, 22, 42, 38], fill=(255, 248, 236, 255))
    sz = int(round(22 * scale_out))
    return hi.resize((sz, sz), LANCZOS)


def make_heart(scale_out):
    art = [".##.##.", "#######", "#######", ".#####.", "..###..", "...#..."]
    sc = max(1, int(round(2 * scale_out)))
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
    d.rounded_rectangle([0, 0, bw - 1, bh - 1], radius=8 * S, fill=(*C_BUBBLE_BG, 255), outline=(*C_AMBER, 255), width=4)
    mid = bw // 2
    d.polygon([(mid - 10, bh - 4), (mid + 10, bh - 4), (mid, bh + tail_h - 2)], fill=(*C_BUBBLE_BG, 255))
    d.line([(mid - 10, bh - 2), (mid, bh + tail_h - 2), (mid + 10, bh - 2)], fill=(*C_AMBER, 255), width=4)
    draw_centered(d, bw / 2.0, bh / 2.0, text, F_BUBBLE, (250, 246, 238, 255))
    return im


def draw_chapter_pill(frame_1x, num, title, accent):
    d = ImageDraw.Draw(frame_1x, "RGBA")
    f1 = get_font(5.5, bold=True, mono=True)
    f2 = get_font(5.5, bold=False)
    tag = f"0{num}"
    tw = int(d.textbbox((0, 0), tag, font=f1)[2] + 12)
    dw = int(d.textbbox((0, 0), title, font=f2)[2] + 14)
    tot = tw + dw + 12
    px = (OUT_W - tot) // 2
    py = 29
    d.rounded_rectangle([px, py, px + tot, py + 22], radius=11, fill=(28, 30, 36, 255), outline=(66, 70, 80, 255), width=1)
    d.rounded_rectangle([px + 4, py + 4, px + 4 + tw, py + 18], radius=7, fill=(44, 48, 58, 255))
    draw_centered(d, px + 4 + tw / 2.0, py + 11.0, tag, f1, accent)
    draw_centered(d, px + 8 + tw + dw / 2.0, py + 11.0, title, f2, C_TEXT)


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
        "highland_cow": load_pack_anims("highland_cow", 78),
        "dog": load_pack_anims("dog", 60),
        "polar_bear": load_pack_anims("polar_bear", 62),
        "stegosaurus": load_pack_anims("stegosaurus", 64),
    }
    dock_icons = build_easy_dock_icons()
    static_stage = build_static_stage(dock_icons)
    cursor_1x, hot_1x = make_macos_cursor(1.0)
    cursor_2x, hot_2x = make_macos_cursor(2.0)
    ball_1x = make_ball(1.0)
    ball_2x = make_ball(2.0)
    heart_1x = make_heart(1.0)
    heart_2x = make_heart(2.0)
    bubble_build = make_bubble_2x("BUILD PASSED!")
    bubble_play = make_bubble_2x("PLAY TIME!")

    dock_cow_y = DOCK_TOP - packs["highland_cow"]["gy_pt"] + 1.0
    notes_cow_y = R_BOX[1] - packs["highland_cow"]["gy_pt"] + 1.0
    cow_w = packs["highland_cow"]["w_pt"]

    cow_x, cow_y, cow_dir = 438.0, dock_cow_y, 1
    bear_x, bear_dir = 84.0, 1
    dog_x, dog_dir = 780.0, -1
    stego_x, stego_dir = 68.0, 1

    ball_active = False
    ball_x, ball_y, ball_vx, ball_vy = 560.0, 44.0, -2.4, 0.0
    ball_kicked = False

    cur = (710.0, 240.0)
    frames_rgb = []

    for f in range(TOTAL_FRAMES):
        fr = static_stage.copy()

        typed = 0
        if 96 <= f < 138:
            typed = min(len(CMD_REACT), int((f - 96) * 1.18))
        elif f >= 138:
            typed = len(CMD_REACT)

        state = {
            "term_focused": 88 <= f < 170,
            "typed_chars": typed,
            "react_done": f >= 138,
            "ball_dropped": f >= 208,
        }
        draw_terminal_window(fr, state)

        # Background Herd Motion
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
        if stego_x > 225.0:
            stego_dir = -1
        elif stego_x < 34.0:
            stego_dir = 1

        # Scenario 1 (0..84): Drag & Toss Highland Cow onto Apple Notes Titlebar
        cow_mode = "walk"
        if f < 26:
            cow_x += cow_dir * 1.8
            if cow_x > 526.0:
                cow_dir = -1
            elif cow_x < 356.0:
                cow_dir = 1
            cow_y = dock_cow_y
        elif f < 50:
            t = smootherstep((f - 26) / 24.0)
            cow_mode = "drag"
            cow_x = lerp(468.0, 540.0, t)
            cow_y = lerp(dock_cow_y, notes_cow_y - 42.0, t)
        elif f < 60:
            t = (f - 50) / 10.0
            cow_mode = "fall"
            cow_x = lerp(540.0, 560.0, t)
            cow_y = lerp(notes_cow_y - 42.0, notes_cow_y, t * t)
        elif f < 174:
            cow_x += cow_dir * 1.4
            if cow_x > 640.0:
                cow_dir = -1
            elif cow_x < 500.0:
                cow_dir = 1
            cow_y = notes_cow_y
        elif f < 186:
            t = (f - 174) / 12.0
            cow_mode = "fall"
            cow_x = lerp(cow_x, 424.0, t)
            cow_y = lerp(notes_cow_y, dock_cow_y, t * t) - math.sin(t * math.pi) * 26.0
        else:
            cow_y = dock_cow_y

        # Scenario 3 (208..279): Play Mode Bouncy Ball Physics
        if f == 208:
            ball_active = True
            ball_x, ball_y, ball_vx, ball_vy = 568.0, 44.0, -2.1, 0.0
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
                ball_vy = -13.4
                ball_vx = 7.0 if dx_b >= 0 else -7.0
            elif abs(dx_b) > 16.0:
                cow_dir = 1 if dx_b > 0 else -1
                cow_x = clamp(cow_x + cow_dir * 3.4, DOCK_BOX[0] + 8.0, DOCK_BOX[2] - cow_w - 8.0)

        herd_hop = 0.0
        if 138 <= f <= 154:
            herd_hop = -math.sin(((f - 138) / 16.0) * math.pi) * 22.0

        # Smooth Virtual Camera Zoom
        cam_zoom, cam_x, cam_y = 1.0, 480.0, 270.0
        if 84 <= f < 98:
            t = smootherstep((f - 84) / 14.0)
            cam_zoom, cam_x, cam_y = lerp(1.0, 1.36, t), lerp(480.0, 276.0, t), lerp(270.0, 258.0, t)
        elif 98 <= f < 138:
            cam_zoom, cam_x, cam_y = 1.36, 276.0, 258.0
        elif 138 <= f < 152:
            t = smootherstep((f - 138) / 14.0)
            cam_zoom, cam_x, cam_y = lerp(1.36, 1.0, t), lerp(276.0, 480.0, t), lerp(258.0, 270.0, t)
        elif 188 <= f < 198:
            t = smootherstep((f - 188) / 10.0)
            cam_zoom, cam_x, cam_y = lerp(1.0, 1.22, t), lerp(480.0, 500.0, t), lerp(270.0, 395.0, t)
        elif 198 <= f < 210:
            cam_zoom, cam_x, cam_y = 1.22, 500.0, 395.0
        elif 210 <= f < 224:
            t = smootherstep((f - 210) / 14.0)
            cam_zoom, cam_x, cam_y = lerp(1.22, 1.0, t), lerp(500.0, 480.0, t), lerp(395.0, 270.0, t)

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
        elif f < 86:
            t = smootherstep((f - 50) / 22.0)
            cur = (lerp(540.0 + cow_w * 0.5, 250.0, t), lerp(notes_cow_y - 18.0, 300.0, t))
        elif f < 178:
            cur = (250.0, 300.0)
        elif f < 190:
            t = smootherstep((f - 178) / 12.0)
            cur = (lerp(250.0, menu_target[0], t), lerp(300.0, menu_target[1], t))
        elif f < 210:
            hov = 1 if f >= 198 else -1
            btn_x, btn_y = draw_macos_context_menu(fr, menu_target[0] + 6.0, menu_target[1] - 4.0, hov, pressed=(205 <= f <= 209))
            if f < 198:
                t = smootherstep((f - 190) / 8.0)
                cur = (lerp(menu_target[0], btn_x, t), lerp(menu_target[1], btn_y, t))
            else:
                cur = (btn_x, btn_y)
            if 206 <= f < 210:
                rip = (btn_x, btn_y, (f - 206) / 10.0, C_SYS_BLUE)
        else:
            if 210 <= f < 218:
                rip = (cur[0], cur[1], (f - 206) / 10.0, C_SYS_BLUE)

        # Render pets, bubbles, hearts, ball & cursor:
        # When zoom == 1.0, downsample ONLY the vector desktop/windows first, then composite
        # razor-sharp 1x NEAREST sprites directly -> ZERO blurry dino or muddy outlines!
        fi_fast = (f // 4) % 4
        fi_med = (f // 5) % 4
        fi_slow = (f // 6) % 4

        pet_draws = [
            ("polar_bear", bear_x, L_BOX[1] - packs["polar_bear"]["gy_pt"] + 1.0 + herd_hop, bear_dir, "walk", fi_med),
            ("dog", dog_x, R_BOX[1] - packs["dog"]["gy_pt"] + 1.0 + herd_hop, dog_dir, "walk", fi_med),
            ("stegosaurus", stego_x, FLOOR_Y - packs["stegosaurus"]["gy_pt"] + 1.0 + herd_hop, stego_dir, "walk", fi_slow),
            ("highland_cow", cow_x, cow_y + herd_hop, cow_dir, cow_mode, fi_fast),
        ]

        show_react = 138 <= f <= 174
        show_kick = ball_kicked and f < 268
        active_bub = bubble_build if (138 <= f <= 174) else (bubble_play if (ball_kicked and f < 268) else None)

        if cam_zoom <= 1.001:
            out_1x = fr.resize((OUT_W, OUT_H), LANCZOS)
            for pid, px_pt, py_pt, pdir, pmode, pfi in pet_draws:
                pk = packs[pid]
                sp = pk[f"{pmode}_{'r' if pdir > 0 else 'l'}_1x"][pfi % len(pk[f"{pmode}_l_1x"])]
                out_1x.alpha_composite(sp, (round(px_pt), round(py_pt)))

            if show_react or show_kick:
                t0 = 138 if show_react else 238
                p = ((f - t0) % 24) / 24.0
                targets = [(cow_x + cow_w * 0.5, cow_y + herd_hop, 32.0)]
                if show_react:
                    targets.append((bear_x + packs["polar_bear"]["w_pt"] * 0.5, L_BOX[1] - packs["polar_bear"]["gy_pt"] + herd_hop, 16.0))
                    targets.append((dog_x + packs["dog"]["w_pt"] * 0.5, R_BOX[1] - packs["dog"]["gy_pt"] + herd_hop, 16.0))
                for tx, ty, spread in targets:
                    for k, (ox, ph) in enumerate([(-spread, 0.0), (spread - 6.0, 0.40)]):
                        pp = (p + ph) % 1.0
                        hx = round(tx + ox + math.sin(pp * math.tau + k) * 3.5)
                        hy = round(ty + 18.0 - pp * 24.0)
                        if 0 <= hx < OUT_W - 14 and MENUBAR_H <= hy < OUT_H - 12:
                            out_1x.alpha_composite(heart_1x, (hx, hy))

            if active_bub is not None:
                bub1 = active_bub.resize((active_bub.width // 2, active_bub.height // 2), LANCZOS)
                bx = round(clamp(cow_x + cow_w * 0.5 - bub1.width * 0.5, 12.0, OUT_W - bub1.width - 12.0))
                raw_by = round(cow_y + herd_hop + 6.0 - bub1.height)
                min_by = (R_BOX[3] + 6) if (f >= 186) else (MENUBAR_H + 4)
                out_1x.alpha_composite(bub1, (bx, max(min_by, raw_by)))

            if ball_active:
                out_1x.alpha_composite(ball_1x, (round(ball_x), round(ball_y)))
            if rip is not None:
                rx, ry, rp, rcol = rip
                e = ease_out_cubic(rp)
                rr = 4 + 22 * e
                rov = Image.new("RGBA", (OUT_W, OUT_H), (0, 0, 0, 0))
                ImageDraw.Draw(rov, "RGBA").ellipse([rx - rr, ry - rr, rx + rr, ry + rr], outline=(*rcol, int(245 * (1.0 - e))), width=2)
                out_1x.alpha_composite(rov)
            out_1x.alpha_composite(cursor_1x, (round(cur[0] - hot_1x[0]), round(cur[1] - hot_1x[1])))
        else:
            for pid, px_pt, py_pt, pdir, pmode, pfi in pet_draws:
                pk = packs[pid]
                sp = pk[f"{pmode}_{'r' if pdir > 0 else 'l'}_2x"][pfi % len(pk[f"{pmode}_l_2x"])]
                fr.alpha_composite(sp, (round(px_pt * S), round(py_pt * S)))

            if show_react or show_kick:
                t0 = 138 if show_react else 238
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

            if active_bub is not None:
                bx = round(clamp((cow_x + cow_w * 0.5) * S - active_bub.width * 0.5, 24.0, W - active_bub.width - 24.0))
                raw_by = round((cow_y + herd_hop + 6.0) * S - active_bub.height)
                min_by = (R_BOX[3] + 6) * S if (f >= 186) else (MENUBAR_H + 4) * S
                fr.alpha_composite(active_bub, (bx, max(min_by, raw_by)))

            if ball_active:
                fr.alpha_composite(ball_2x, (round(ball_x * S), round(ball_y * S)))
            if rip is not None:
                rx, ry, rp, rcol = rip
                e = ease_out_cubic(rp)
                rr = (4 + 22 * e) * S
                rov = Image.new("RGBA", (W, H), (0, 0, 0, 0))
                ImageDraw.Draw(rov, "RGBA").ellipse([rx * S - rr, ry * S - rr, rx * S + rr, ry * S + rr], outline=(*rcol, int(245 * (1.0 - e))), width=4)
                fr.alpha_composite(rov)
            fr.alpha_composite(cursor_2x, (round(cur[0] * S - hot_2x[0]), round(cur[1] * S - hot_2x[1])))
            out_1x = apply_camera(fr, cam_zoom, cam_x, cam_y)

        if f < 84:
            draw_chapter_pill(out_1x, 1, "Drag, Toss & Walk on Any Window", C_CYAN)
        elif f < 178:
            draw_chapter_pill(out_1x, 2, "AI Agent MCP & Speech Bubbles", C_GREEN)
        else:
            draw_chapter_pill(out_1x, 3, "Play Mode · Bouncy Ball & Herd Chase", C_SYS_ORANGE)

        frames_rgb.append(out_1x.convert("RGB"))

    # Seamless 10-frame crossfade back to Frame 0
    fade_len = 10
    first_rgb = frames_rgb[0]
    for i in range(fade_len):
        idx = len(frames_rgb) - fade_len + i
        frames_rgb[idx] = Image.blend(frames_rgb[idx], first_rgb, ease_out_cubic((i + 1) / float(fade_len)))

    return frames_rgb, packs


def harvest_exact_sprite_colors(packs):
    """Extract the exact un-bucketed dominant RGB colors from all 4 sprites (keeps Stego green 100% intact!)."""
    out = []
    for pid in ("stegosaurus", "highland_cow", "dog", "polar_bear"):
        im = packs[pid]["sample_tile"].convert("RGBA")
        counts = {}
        for r, g, b, a in im.getdata():
            if a < 220:
                continue
            counts[(r, g, b)] = counts.get((r, g, b), 0) + 1
        top = sorted(counts.items(), key=lambda kv: -kv[1])[:12]
        out.extend([rgb for rgb, _ in top])
    return out


def save_outputs(frames_rgb, packs):
    os.makedirs(os.path.dirname(OUT_GIF), exist_ok=True)
    n = len(frames_rgb)

    sample_indices = [0, 36, 65, 118, 148, 202, 228, 254]
    ui_pinned = [
        C_DESKTOP, C_SHADOW_1, C_SHADOW_2, C_MENUBAR, C_WIN_BG, C_WIN_BAR, C_WIN_BORDER,
        C_NOTES_SIDE, C_NOTES_SEL, C_SYS_YELLOW, C_SYS_ORANGE, C_SYS_BLUE, C_SYS_RED,
        C_TL_RED, C_TL_YELLOW, C_TL_GREEN, C_DOCK_BG, C_DOCK_BORDER,
        C_TEXT, C_MUTED, C_GREEN, C_CYAN, C_AMBER, C_BUBBLE_BG,
        C_HEART, C_HEART_HI, C_BALL, C_BALL_STRIPE,
        (246, 246, 248), (108, 196, 252), (64, 160, 238), (252, 240, 222),
    ]
    sprite_pinned = harvest_exact_sprite_colors(packs)
    pinned = ui_pinned + sprite_pinned

    # Heavy swatch area (240px = 4 rows x 60px) guarantees ZERO palette starvation on any sprite or accent!
    swatch_rows = 4
    row_h = 60
    swatch_h = swatch_rows * row_h
    strip = Image.new("RGB", (OUT_W, OUT_H * len(sample_indices) + swatch_h), C_DESKTOP)
    for i, idx in enumerate(sample_indices):
        strip.paste(frames_rgb[min(idx, n - 1)], (0, i * OUT_H))

    sd = ImageDraw.Draw(strip)
    sy = OUT_H * len(sample_indices)
    chunk = (len(pinned) + swatch_rows - 1) // swatch_rows
    for r_i in range(swatch_rows):
        row_cols = pinned[r_i * chunk:(r_i + 1) * chunk]
        if not row_cols:
            continue
        sw = max(1, OUT_W // len(row_cols))
        y0 = sy + r_i * row_h
        y1 = y0 + row_h
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
    print(f"Saved Vibrant Launch GIF: {OUT_GIF} ({OUT_W}x{OUT_H}, {n} frames @ {FPS} FPS, {os.path.getsize(OUT_GIF)/1024:.1f} KB)")

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