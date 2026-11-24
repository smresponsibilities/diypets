// DIYPets Sprite Sheet Slicer + 2x EPX Inverse Rigid MLS (Schaefer 2006) Auto-Rigger
// + Interactive Draggable Skeleton Pins + Lossless GIF89a & PNG Sprite Sheet Exporter

let gifCustomRig = null;
let gifCustomSheet = null;
let gifCustomRawImg = null;
let gifCustomName = "";
let gifPreviewTick = 0;
let gifPreviewTimer = null;
let gifDragPinIdx = -1;
let gifPaused = false;
let gifLastCanvasGeom = null;

function isNeutralGrey(r, g, b) {
  const mx = Math.max(r, g, b), mn = Math.min(r, g, b);
  const sat = mx - mn, lum = (r + g + b) / 3;
  return sat <= 14 && lum >= 92 && lum <= 224 && (b - r) <= 6;
}

function removeFloatingSpecks(data, w, h) {
  const n = w * h;
  const vis = new Uint8Array(n);
  const q = new Int32Array(n);
  const comps = [];
  let maxSize = 0;
  for (let i = 0; i < n; i++) {
    if (vis[i] || data[i * 4 + 3] === 0) continue;
    let head = 0, tail = 0;
    vis[i] = 1; q[tail++] = i;
    while (head < tail) {
      const cur = q[head++];
      const cx = cur % w, cy = (cur / w) | 0;
      if (cx > 0 && !vis[cur - 1] && data[(cur - 1) * 4 + 3] > 0) { vis[cur - 1] = 1; q[tail++] = cur - 1; }
      if (cx + 1 < w && !vis[cur + 1] && data[(cur + 1) * 4 + 3] > 0) { vis[cur + 1] = 1; q[tail++] = cur + 1; }
      if (cy > 0 && !vis[cur - w] && data[(cur - w) * 4 + 3] > 0) { vis[cur - w] = 1; q[tail++] = cur - w; }
      if (cy + 1 < h && !vis[cur + w] && data[(cur + w) * 4 + 3] > 0) { vis[cur + w] = 1; q[tail++] = cur + w; }
    }
    if (tail > maxSize) maxSize = tail;
    comps.push(q.slice(0, tail));
  }
  const minKeep = Math.max(4, Math.floor(maxSize * 0.004));
  for (let c = 0; c < comps.length; c++) {
    if (comps[c].length < minKeep) {
      const arr = comps[c];
      for (let k = 0; k < arr.length; k++) {
        const p = arr[k] * 4;
        data[p] = 0; data[p + 1] = 0; data[p + 2] = 0; data[p + 3] = 0;
      }
    }
  }
}

function cleanRgbaPixels(data, w, h, opts) {
  const out = new Uint8ClampedArray(data);
  const n = w * h;
  const tol = (opts && typeof opts.tolerance === "number") ? opts.tolerance : 40;
  const keyColor = (opts && Array.isArray(opts.keyColor) && opts.keyColor.length === 3) ? opts.keyColor : null;

  // Pass 1: Border flood-fill removal for opaque/checkerboard/chroma-key outer backgrounds
  const corners = [0, w - 1, (h - 1) * w, (h - 1) * w + (w - 1)];
  const bgSamples = [];
  if (keyColor) {
    bgSamples.push([keyColor[0], keyColor[1], keyColor[2]]);
  } else {
    for (let i = 0; i < corners.length; i++) {
      const p = corners[i] * 4;
      if (out[p + 3] >= 24) bgSamples.push([out[p], out[p + 1], out[p + 2]]);
    }
  }
  const matchOuterBg = (p) => {
    if (out[p + 3] < 32) return true;
    const r = out[p], g = out[p + 1], b = out[p + 2];
    if (!keyColor && bgSamples.length > 0 && isNeutralGrey(r, g, b)) return true;
    for (let i = 0; i < bgSamples.length; i++) {
      const s = bgSamples[i];
      if (Math.abs(r - s[0]) + Math.abs(g - s[1]) + Math.abs(b - s[2]) <= tol) return true;
    }
    return false;
  };
  if (bgSamples.length > 0) {
    const vis = new Uint8Array(n);
    const q = new Int32Array(n);
    let head = 0, tail = 0;
    const push = (idx) => {
      if (!vis[idx] && matchOuterBg(idx * 4)) { vis[idx] = 1; q[tail++] = idx; }
    };
    for (let x = 0; x < w; x++) { push(x); push((h - 1) * w + x); }
    for (let y = 0; y < h; y++) { push(y * w); push(y * w + (w - 1)); }
    while (head < tail) {
      const cur = q[head++];
      const p = cur * 4;
      out[p] = 0; out[p + 1] = 0; out[p + 2] = 0; out[p + 3] = 0;
      const cx = cur % w, cy = (cur / w) | 0;
      if (cx > 0) push(cur - 1);
      if (cx + 1 < w) push(cur + 1);
      if (cy > 0) push(cur - w);
      if (cy + 1 < h) push(cur + w);
    }
  }

  if (keyColor) {
    const kTol = tol * 1.15;
    for (let i = 0; i < n; i++) {
      const p = i * 4;
      if (out[p + 3] === 0) continue;
      const d = Math.abs(out[p] - keyColor[0]) + Math.abs(out[p + 1] - keyColor[1]) + Math.abs(out[p + 2] - keyColor[2]);
      if (d <= kTol) {
        out[p] = 0; out[p + 1] = 0; out[p + 2] = 0; out[p + 3] = 0;
      }
    }
  }

  // Pass 2: Hard alpha matte (eliminates semi-transparent grey anti-aliased fringes)
  for (let i = 0; i < n; i++) {
    const p = i * 4, a = out[p + 3];
    if (a < 180 || (a < 250 && isNeutralGrey(out[p], out[p + 1], out[p + 2]))) {
      out[p] = 0; out[p + 1] = 0; out[p + 2] = 0; out[p + 3] = 0;
    } else {
      out[p + 3] = 255;
    }
  }

  // Pass 3: Strip neutral-grey outer fringes & trapped checkerboard pockets
  const visPocket = new Uint8Array(n);
  const q2 = new Int32Array(n);
  for (let i = 0; i < n; i++) {
    if (visPocket[i] || out[i * 4 + 3] === 0) continue;
    const p0 = i * 4;
    if (!isNeutralGrey(out[p0], out[p0 + 1], out[p0 + 2])) continue;
    let head = 0, tail = 0, touchesEmpty = 0, touchesDark = 0;
    visPocket[i] = 1; q2[tail++] = i;
    while (head < tail) {
      const cur = q2[head++];
      const cx = cur % w, cy = (cur / w) | 0;
      const nbrs = [
        cx > 0 ? cur - 1 : -1, cx + 1 < w ? cur + 1 : -1,
        cy > 0 ? cur - w : -1, cy + 1 < h ? cur + w : -1,
      ];
      for (let k = 0; k < 4; k++) {
        const nb = nbrs[k];
        if (nb < 0) { touchesEmpty++; continue; }
        const np = nb * 4;
        if (out[np + 3] === 0) { touchesEmpty++; continue; }
        if (isNeutralGrey(out[np], out[np + 1], out[np + 2])) {
          if (!visPocket[nb]) { visPocket[nb] = 1; q2[tail++] = nb; }
        } else if ((out[np] + out[np + 1] + out[np + 2]) / 3 < 85) {
          touchesDark++;
        }
      }
    }
    let sumLum = 0;
    for (let k = 0; k < tail; k++) {
      const pk = q2[k] * 4;
      sumLum += out[pk] * 0.299 + out[pk + 1] * 0.587 + out[pk + 2] * 0.114;
    }
    const avgLum = tail > 0 ? (sumLum / tail) : 0;
    const isOuter = touchesEmpty > 0 && tail <= Math.max(24, n * 0.04);
    // Only strip trapped interior pockets if they are large bright white/checkerboard cutouts (never tiny grey shading in eyes/mouth/paws!)
    const isTrapped = touchesEmpty === 0 && touchesDark >= 8 && tail >= 18 && tail <= Math.max(48, n * 0.03) && avgLum >= 215;
    if (isOuter || isTrapped) {
      for (let k = 0; k < tail; k++) {
        const p = q2[k] * 4;
        out[p] = 0; out[p + 1] = 0; out[p + 2] = 0; out[p + 3] = 0;
      }
    }
  }

  // Pass 4: Remove tiny stray air dust specks (like floating 1-2px dots outside the sprite)
  removeFloatingSpecks(out, w, h);
  return out;
}

// Run-length pixel-block scale detector: finds true pixel size (1..24) on high-res upscaled pixel art
// WITHOUT artificially crushing an 84x76 sprite down to 42x38!
// Strict intra-block uniformity detector:
// Only reports block scale k > 1 when >= 92% of opaque k x k tiles are flat (true upscaled pixel art).
// Native-resolution sprites with curved 1-2px outlines or subtle shading return 1 (zero downsampling blur!).
function detectNativeBlockScale(rgba, w, h, bbOpt) {
  const bb = bbOpt || findBbox(rgba, w, h);
  const bw = bb.bw, bh = bb.bh;
  if (bw < 16 || bh < 16) return 1;
  const maxK = Math.min(8, Math.floor(Math.min(bw, bh) / 16));
  for (let k = maxK; k >= 2; k--) {
    let tiles = 0, flatTiles = 0;
    const stepX = Math.max(1, Math.floor(bw / (k * 28)));
    const stepY = Math.max(1, Math.floor(bh / (k * 28)));
    for (let ty = 0; ty + k <= bh; ty += k * stepY) {
      for (let tx = 0; tx + k <= bw; tx += k * stepX) {
        const x0 = bb.minX + tx, y0 = bb.minY + ty;
        const p0 = (y0 * w + x0) * 4;
        const a0 = rgba[p0 + 3];
        if (a0 < 128) continue;
        tiles++;
        const r0 = rgba[p0], g0 = rgba[p0 + 1], b0 = rgba[p0 + 2];
        let flat = true;
        for (let dy = 0; dy < k && flat; dy++) {
          for (let dx = 0; dx < k; dx++) {
            const p = ((y0 + dy) * w + (x0 + dx)) * 4;
            if (
              Math.abs(rgba[p + 3] - a0) > 8 ||
              Math.abs(rgba[p] - r0) > 2 ||
              Math.abs(rgba[p + 1] - g0) > 2 ||
              Math.abs(rgba[p + 2] - b0) > 2
            ) {
              flat = false;
              break;
            }
          }
        }
        if (flat) flatTiles++;
      }
    }
    if (tiles >= 12 && (flatTiles / tiles) >= 0.92) {
      return k;
    }
  }
  return 1;
}

function findBbox(grid, w, h) {
  let minX = w, minY = h, maxX = -1, maxY = -1;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (grid[(y * w + x) * 4 + 3] > 0) {
        if (x < minX) minX = x; if (x > maxX) maxX = x;
        if (y < minY) minY = y; if (y > maxY) maxY = y;
      }
    }
  }
  if (maxX < minX) return { minX: 0, minY: 0, maxX: w - 1, maxY: h - 1, bw: w, bh: h };
  return { minX, minY, maxX, maxY, bw: maxX - minX + 1, bh: maxY - minY + 1 };
}

// Extract tight sprite on its TRUE native pixel grid (up to 112px) + generous transparent headroom
// so Hop, Walk, and Run NEVER clip ears, tails, or paws!
function extractPaddedNativeGrid(rgba, w, h) {
  const bb = findBbox(rgba, w, h);
  if (!bb) {
    return {
      grid: new Uint8ClampedArray(4),
      w: 1,
      h: 1,
      nativeW: 1,
      nativeH: 1,
      scale: 1,
      blockScale: 1,
      padTop: 0,
      padSide: 0,
    };
  }

  const rawBlock = detectNativeBlockScale(rgba, w, h, bb);
  const maxDim = 160;
  let scaleF = rawBlock;
  let nw = Math.max(1, Math.round(bb.bw / scaleF));
  let nh = Math.max(1, Math.round(bb.bh / scaleF));
  if (Math.max(nw, nh) > maxDim) {
    scaleF = Math.max(bb.bw, bb.bh) / maxDim;
    nw = Math.max(1, Math.round(bb.bw / scaleF));
    nh = Math.max(1, Math.round(bb.bh / scaleF));
  }

  // Subpixel Grid Phase-Lock: require non-default phase to beat 0.5 center by >= 12%
  let phaseX = 0.5, phaseY = 0.5;
  if (scaleF >= 2) {
    const steps = Math.min(8, Math.max(2, Math.round(scaleF)));
    let bestEx = -1, bestEy = -1;
    for (let s = 0; s < steps; s++) {
      const off = s / steps;
      let ex = 0, ey = 0;
      for (let x = 1; x < nw; x++) {
        const bx = Math.min(w - 1, Math.max(1, bb.minX + Math.floor((x + off) * scaleF)));
        for (let y = 0; y < nh; y += 2) {
          const by = Math.min(h - 1, bb.minY + Math.floor((y + 0.5) * scaleF));
          const p0 = (by * w + bx - 1) * 4, p1 = (by * w + bx) * 4;
          ex += Math.abs(rgba[p0] - rgba[p1]) + Math.abs(rgba[p0 + 1] - rgba[p1 + 1]) + Math.abs(rgba[p0 + 2] - rgba[p1 + 2]) + Math.abs(rgba[p0 + 3] - rgba[p1 + 3]);
        }
      }
      if (s === 0) { bestEx = ex * 1.12; phaseX = 0.5; }
      else if (ex > bestEx) { bestEx = ex; phaseX = (off + 0.5) % 1.0; }

      for (let y = 1; y < nh; y++) {
        const by = Math.min(h - 1, Math.max(1, bb.minY + Math.floor((y + off) * scaleF)));
        for (let x = 0; x < nw; x += 2) {
          const bx = Math.min(w - 1, bb.minX + Math.floor((x + 0.5) * scaleF));
          const p0 = ((by - 1) * w + bx) * 4, p1 = (by * w + bx) * 4;
          ey += Math.abs(rgba[p0] - rgba[p1]) + Math.abs(rgba[p0 + 1] - rgba[p1 + 1]) + Math.abs(rgba[p0 + 2] - rgba[p1 + 2]) + Math.abs(rgba[p0 + 3] - rgba[p1 + 3]);
        }
      }
      if (s === 0) { bestEy = ey * 1.12; phaseY = 0.5; }
      else if (ey > bestEy) { bestEy = ey; phaseY = (off + 0.5) % 1.0; }
    }
  }

  const padSide = Math.max(4, Math.min(14, Math.ceil(nw * 0.10)));
  const padTop = Math.max(4, Math.min(18, Math.ceil(nh * 0.12)));
  const outW = nw + padSide * 2;
  const outH = nh + padTop;
  const grid = new Uint8ClampedArray(outW * outH * 4);

  for (let y = 0; y < nh; y++) {
    const sy = Math.min(h - 1, bb.minY + Math.floor((y + phaseY) * scaleF));
    for (let x = 0; x < nw; x++) {
      const sx = Math.min(w - 1, bb.minX + Math.floor((x + phaseX) * scaleF));
      const sp = (sy * w + sx) * 4;
      const dp = ((y + padTop) * outW + (x + padSide)) * 4;
      const a = rgba[sp + 3];
      const lum = rgba[sp] * 0.299 + rgba[sp + 1] * 0.587 + rgba[sp + 2] * 0.114;
      if (a >= 215 || (a >= 148 && lum < 75)) {
        grid[dp] = rgba[sp];
        grid[dp + 1] = rgba[sp + 1];
        grid[dp + 2] = rgba[sp + 2];
        grid[dp + 3] = 255;
      }
    }
  }

  return {
    grid,
    w: outW,
    h: outH,
    nativeW: nw,
    nativeH: nh,
    scale: scaleF,
    blockScale: rawBlock,
    padTop,
    padSide,
  };
}

// Strip exterior grey anti-aliasing halo (neutral grey pixels touching transparency & a dark outline)
// plus detached 1-5px floating dust specks without ever touching interior shading or dark outlines.
function stripExteriorGreyHalo(grid, w, h, passesOpt) {
  const passes = typeof passesOpt === "number" ? Math.max(0, Math.min(4, Math.round(passesOpt))) : 2;
  for (let pass = 0; pass < passes; pass++) {
    const snap = new Uint8ClampedArray(grid);
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const idx = y * w + x;
        const p = idx * 4;
        if (snap[p + 3] === 0) continue;
        const r = snap[p], g = snap[p + 1], b = snap[p + 2];
        const sat = Math.max(r, g, b) - Math.min(r, g, b);
        const lum = r * 0.299 + g * 0.587 + b * 0.114;
        if (sat > 12 || lum < 90 || lum > 210) continue;

        let touchesTrans = false;
        let touchesDark = false;
        const dirs = [[-1, 0], [1, 0], [0, -1], [0, 1], [-1, -1], [1, -1], [-1, 1], [1, 1]];
        for (let k = 0; k < 8; k++) {
          const nx = x + dirs[k][0], ny = y + dirs[k][1];
          if (nx < 0 || nx >= w || ny < 0 || ny >= h) {
            if (k < 4) touchesTrans = true;
            continue;
          }
          const np = (ny * w + nx) * 4;
          if (snap[np + 3] === 0) {
            if (k < 4) touchesTrans = true;
          } else {
            const nlum = snap[np] * 0.299 + snap[np + 1] * 0.587 + snap[np + 2] * 0.114;
            if (nlum < 68) touchesDark = true;
          }
        }
        if (touchesTrans && touchesDark) {
          grid[p] = 0; grid[p + 1] = 0; grid[p + 2] = 0; grid[p + 3] = 0;
        }
      }
    }
  }

  // Remove detached floating dust specks (8-connected components <= 5 pixels)
  const visited = new Uint8Array(w * h);
  const q = new Int32Array(w * h);
  const comp = [];
  for (let i = 0; i < w * h; i++) {
    if (visited[i] || grid[i * 4 + 3] === 0) continue;
    let head = 0, tail = 0;
    q[tail++] = i;
    visited[i] = 1;
    comp.length = 0;
    while (head < tail) {
      const u = q[head++];
      comp.push(u);
      const ux = u % w, uy = (u / w) | 0;
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          if (dx === 0 && dy === 0) continue;
          const nx = ux + dx, ny = uy + dy;
          if (nx < 0 || nx >= w || ny < 0 || ny >= h) continue;
          const v = ny * w + nx;
          if (!visited[v] && grid[v * 4 + 3] > 0) {
            visited[v] = 1;
            q[tail++] = v;
          }
        }
      }
    }
    if (comp.length <= 5) {
      for (let j = 0; j < comp.length; j++) {
        const p = comp[j] * 4;
        grid[p] = 0; grid[p + 1] = 0; grid[p + 2] = 0; grid[p + 3] = 0;
      }
    }
  }
}

function upscale2xEpx(src, w, h) {
  const dw = w * 2, dh = h * 2;
  const dst = new Uint8ClampedArray(dw * dh * 4);
  const eq = (a, b) =>
    src[a + 3] === src[b + 3] &&
    Math.abs(src[a] - src[b]) <= 8 &&
    Math.abs(src[a + 1] - src[b + 1]) <= 8 &&
    Math.abs(src[a + 2] - src[b + 2]) <= 8;
  const copy = (sp, dp) => {
    dst[dp] = src[sp]; dst[dp + 1] = src[sp + 1]; dst[dp + 2] = src[sp + 2]; dst[dp + 3] = src[sp + 3];
  };
  for (let y = 0; y < h; y++) {
    const yu = Math.max(0, y - 1), yd = Math.min(h - 1, y + 1);
    for (let x = 0; x < w; x++) {
      const xl = Math.max(0, x - 1), xr = Math.min(w - 1, x + 1);
      const P = (y * w + x) * 4;
      const A = (yu * w + x) * 4;
      const B = (y * w + xr) * 4;
      const C = (y * w + xl) * 4;
      const D = (yd * w + x) * 4;
      const p00 = ((y * 2) * dw + (x * 2)) * 4;
      const p01 = ((y * 2) * dw + (x * 2 + 1)) * 4;
      const p10 = ((y * 2 + 1) * dw + (x * 2)) * 4;
      const p11 = ((y * 2 + 1) * dw + (x * 2 + 1)) * 4;
      copy((eq(C, A) && !eq(C, D) && !eq(A, B)) ? A : P, p00);
      copy((eq(A, B) && !eq(A, C) && !eq(B, D)) ? B : P, p01);
      copy((eq(D, C) && !eq(D, B) && !eq(C, A)) ? C : P, p10);
      copy((eq(B, D) && !eq(B, A) && !eq(D, C)) ? D : P, p11);
    }
  }
  return { data: dst, w: dw, h: dh };
}

function detectSpriteSheetGrid(rgba, w, h) {
  if (w >= h * 1.85) {
    const estCols = Math.round(w / h);
    if (estCols >= 2 && estCols <= 16 && Math.abs((w / estCols) - h) <= h * 0.25) {
      return { isSheet: true, cols: estCols, rows: 1 };
    }
  }
  const colCands = [16, 12, 10, 9, 8, 6, 5, 4, 3, 2];
  const rowCands = [8, 7, 6, 5, 4, 3, 2, 1];
  for (let ri = 0; ri < rowCands.length; ri++) {
    const rows = rowCands[ri];
    if (h % rows !== 0) continue;
    const th = h / rows;
    if (th < 24) continue;
    for (let ci = 0; ci < colCands.length; ci++) {
      const cols = colCands[ci];
      if (cols * rows < 2 || w % cols !== 0) continue;
      const tw = w / cols;
      if (tw < 24 || Math.abs(tw - th) > Math.max(tw, th) * 0.3) continue;
      let seamChecked = 0, seamClear = 0;
      for (let c = 1; c < cols; c++) {
        const gx = c * tw;
        for (let y = 0; y < h; y += 2) {
          seamChecked++;
          if (rgba[(y * w + gx) * 4 + 3] < 24 && rgba[(y * w + gx - 1) * 4 + 3] < 24) seamClear++;
        }
      }
      for (let r = 1; r < rows; r++) {
        const gy = r * th;
        for (let x = 0; x < w; x += 2) {
          seamChecked++;
          if (rgba[(gy * w + x) * 4 + 3] < 24 && rgba[((gy - 1) * w + x) * 4 + 3] < 24) seamClear++;
        }
      }
      if (seamChecked > 0 && (seamClear / seamChecked) >= 0.94) {
        return { isSheet: true, cols, rows };
      }
    }
  }
  return { isSheet: false, cols: 1, rows: 1 };
}

function parseFrameSelection(spec, total) {
  const clean = (spec || "").trim().toLowerCase();
  if (!clean || clean === "all") {
    const all = [];
    for (let i = 0; i < total; i++) all.push(i);
    return all;
  }
  const out = [];
  const parts = clean.split(",");
  for (let i = 0; i < parts.length; i++) {
    const tok = parts[i].trim();
    if (!tok) continue;
    if (tok.indexOf("-") !== -1) {
      const ab = tok.split("-");
      const a = Math.max(0, parseInt(ab[0], 10) || 0);
      const b = Math.min(total - 1, parseInt(ab[1], 10) || (total - 1));
      for (let k = a; k <= b; k++) out.push(k);
    } else {
      const idx = parseInt(tok, 10);
      if (!isNaN(idx) && idx >= 0 && idx < total) out.push(idx);
    }
  }
  return out.length > 0 ? out : [0];
}

function sliceSpriteSheet(rawRgba, w, h, cols, rows, cleanBg, frameSpec) {
  const tw = Math.max(1, Math.floor(w / Math.max(1, cols)));
  const th = Math.max(1, Math.floor(h / Math.max(1, rows)));
  const tiles = [];
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const tile = new Uint8ClampedArray(tw * th * 4);
      let opaqueCount = 0;
      for (let y = 0; y < th; y++) {
        const sy = r * th + y;
        for (let x = 0; x < tw; x++) {
          const sx = c * tw + x;
          const sp = (sy * w + sx) * 4, dp = (y * tw + x) * 4;
          tile[dp] = rawRgba[sp]; tile[dp + 1] = rawRgba[sp + 1];
          tile[dp + 2] = rawRgba[sp + 2]; tile[dp + 3] = rawRgba[sp + 3];
          if (rawRgba[sp + 3] > 24) opaqueCount++;
        }
      }
      if (opaqueCount > 8) {
        tiles.push({ rgba: cleanBg ? cleanRgbaPixels(tile, tw, th) : tile, w: tw, h: th });
      }
    }
  }
  if (tiles.length === 0) return null;

  let targetFootY = 0;
  const footYs = [];
  for (let i = 0; i < tiles.length; i++) {
    const bb = findBbox(tiles[i].rgba, tw, th);
    footYs.push(bb.maxY);
    if (bb.maxY > targetFootY) targetFootY = bb.maxY;
  }
  for (let i = 0; i < tiles.length; i++) {
    const shiftY = targetFootY - footYs[i];
    if (shiftY > 0 && shiftY <= Math.floor(th * 0.25)) {
      const shifted = new Uint8ClampedArray(tw * th * 4);
      for (let y = 0; y < th - shiftY; y++) {
        for (let x = 0; x < tw; x++) {
          const sp = (y * tw + x) * 4, dp = ((y + shiftY) * tw + x) * 4;
          shifted[dp] = tiles[i].rgba[sp]; shifted[dp + 1] = tiles[i].rgba[sp + 1];
          shifted[dp + 2] = tiles[i].rgba[sp + 2]; shifted[dp + 3] = tiles[i].rgba[sp + 3];
        }
      }
      tiles[i].rgba = shifted;
    }
  }
  return { frames: tiles, cols, rows, tw, th, activeIndices: parseFrameSelection(frameSpec, tiles.length) };
}


function detectDarkInk(grid, w, h) {
  let best = [20, 22, 28];
  let bestLum = 95;
  for (let y = 1; y < h - 1; y++) {
    for (let x = 1; x < w - 1; x++) {
      const idx = y * w + x;
      const p = idx * 4;
      if (grid[p + 3] === 0) continue;
      if (
        grid[(idx - 1) * 4 + 3] === 0 || grid[(idx + 1) * 4 + 3] === 0 ||
        grid[(idx - w) * 4 + 3] === 0 || grid[(idx + w) * 4 + 3] === 0
      ) {
        const lum = grid[p] * 0.299 + grid[p + 1] * 0.587 + grid[p + 2] * 0.114;
        if (lum < bestLum) {
          bestLum = lum;
          best = [grid[p], grid[p + 1], grid[p + 2]];
        }
      }
    }
  }
  return best;
}

// Repairs 1px outline gaps opened by Rigid MLS warp without thickening existing dark outlines
function applyCrispOutlineLock(rgba, w, h, ink) {
  const snap = new Uint8ClampedArray(rgba);
  for (let y = 1; y < h - 1; y++) {
    for (let x = 1; x < w - 1; x++) {
      const idx = y * w + x;
      const p = idx * 4;
      if (snap[p + 3] > 0) continue;
      // Check if an empty pixel borders an exposed BRIGHT interior pixel (lum >= 125)
      let brightN = 0;
      const nbrs = [idx - 1, idx + 1, idx - w, idx + w];
      for (let k = 0; k < 4; k++) {
        const np = nbrs[k] * 4;
        if (snap[np + 3] > 0) {
          const lum = snap[np] * 0.299 + snap[np + 1] * 0.587 + snap[np + 2] * 0.114;
          if (lum >= 125) brightN++;
        }
      }
      if (brightN >= 1) {
        rgba[p] = ink[0];
        rgba[p + 1] = ink[1];
        rgba[p + 2] = ink[2];
        rgba[p + 3] = 255;
      }
    }
  }
}

function findColumnMassCenter(grid, w, x0, x1, y0, y1, fallbackX, fallbackY) {
  let sx = 0, sy = 0, cnt = 0;
  const h = grid.length / (4 * w);
  for (let y = Math.max(0, Math.floor(y0)); y <= Math.min(h - 1, Math.floor(y1)); y++) {
    for (let x = Math.max(0, Math.floor(x0)); x <= Math.min(w - 1, Math.floor(x1)); x++) {
      if (grid[(y * w + x) * 4 + 3] > 0) { sx += x; sy += y; cnt++; }
    }
  }
  return cnt > 0 ? { x: sx / cnt, y: sy / cnt } : { x: fallbackX, y: fallbackY };
}


// 2D Medial-Axis Skeleton Extraction (Chamfer Distance Transform + Zhang-Suen Thinning)
// Finds true torso core & real silhouette limb/tail/head endpoints instead of blind bbox percentages.
function extractMedialAxisFeatures(grid, w, h, bbox) {
  const n = w * h;
  const mask = new Uint8Array(n);
  const dist = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    if (grid[i * 4 + 3] > 0) {
      mask[i] = 1;
      dist[i] = 1e6;
    }
  }

  // 2-pass Chamfer Distance Transform (core radius & true torso center)
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = y * w + x;
      if (!mask[i]) continue;
      let d = dist[i];
      if (x === 0 || y === 0) d = 1;
      else {
        d = Math.min(d, dist[i - 1] + 1, dist[i - w] + 1, dist[i - w - 1] + 1.414);
        if (x + 1 < w) d = Math.min(d, dist[i - w + 1] + 1.414);
      }
      dist[i] = d;
    }
  }
  let coreX = (bbox.minX + bbox.maxX) * 0.5;
  let coreY = (bbox.minY + bbox.maxY) * 0.5;
  let coreR = 0;
  for (let y = h - 1; y >= 0; y--) {
    for (let x = w - 1; x >= 0; x--) {
      const i = y * w + x;
      if (!mask[i]) continue;
      let d = dist[i];
      if (x === w - 1 || y === h - 1) d = Math.min(d, 1);
      else {
        d = Math.min(d, dist[i + 1] + 1, dist[i + w] + 1, dist[i + w + 1] + 1.414);
        if (x > 0) d = Math.min(d, dist[i + w - 1] + 1.414);
      }
      dist[i] = d;
      if (d > coreR) { coreR = d; coreX = x; coreY = y; }
    }
  }

  // Zhang-Suen Thinning (fast 2-subiteration morphological skeleton)
  const skel = new Uint8Array(mask);
  const toDel = [];
  const x0 = Math.max(1, bbox.minX), x1 = Math.min(w - 2, bbox.maxX);
  const y0 = Math.max(1, bbox.minY), y1 = Math.min(h - 2, bbox.maxY);

  for (let iter = 0; iter < 36; iter++) {
    let changed = false;
    for (let sub = 0; sub < 2; sub++) {
      toDel.length = 0;
      for (let y = y0; y <= y1; y++) {
        const row = y * w;
        for (let x = x0; x <= x1; x++) {
          const idx = row + x;
          if (!skel[idx]) continue;
          const p2 = skel[idx - w];
          const p3 = skel[idx - w + 1];
          const p4 = skel[idx + 1];
          const p5 = skel[idx + w + 1];
          const p6 = skel[idx + w];
          const p7 = skel[idx + w - 1];
          const p8 = skel[idx - 1];
          const p9 = skel[idx - w - 1];
          const B = p2 + p3 + p4 + p5 + p6 + p7 + p8 + p9;
          if (B < 2 || B > 6) continue;
          const A =
            (!p2 && p3 ? 1 : 0) + (!p3 && p4 ? 1 : 0) + (!p4 && p5 ? 1 : 0) + (!p5 && p6 ? 1 : 0) +
            (!p6 && p7 ? 1 : 0) + (!p7 && p8 ? 1 : 0) + (!p8 && p9 ? 1 : 0) + (!p9 && p2 ? 1 : 0);
          if (A !== 1) continue;
          if (sub === 0) {
            if (p2 * p4 * p6 !== 0 || p4 * p6 * p8 !== 0) continue;
          } else {
            if (p2 * p4 * p8 !== 0 || p2 * p6 * p8 !== 0) continue;
          }
          toDel.push(idx);
        }
      }
      if (toDel.length > 0) {
        changed = true;
        for (let k = 0; k < toDel.length; k++) skel[toDel[k]] = 0;
      }
    }
    if (!changed) break;
  }

  // Collect skeleton points and true anatomical endpoints (degree <= 1)
  const points = [];
  const endpoints = [];
  for (let y = y0; y <= y1; y++) {
    for (let x = x0; x <= x1; x++) {
      const idx = y * w + x;
      if (!skel[idx]) continue;
      points.push([x, y]);
      const deg =
        skel[idx - w] + skel[idx - w + 1] + skel[idx + 1] + skel[idx + w + 1] +
        skel[idx + w] + skel[idx + w - 1] + skel[idx - 1] + skel[idx - w - 1];
      if (deg <= 1) {
        endpoints.push({ x, y, r: dist[idx], dCore: Math.hypot(x - coreX, y - coreY) });
      }
    }
  }
  return { core: { x: coreX, y: coreY, r: coreR }, points, endpoints };
}

// Seamless periodic damped spring-mass solver for secondary motion (Willett et al., SIGGRAPH 2017)
// Runs 2 full warmup cycles so frame 0 wraps cleanly into frame N-1 with zero loop discontinuity.
function computeSpringFollowThrough(primaryDeltas, stiffness, damping) {
  const n = primaryDeltas.length;
  if (n === 0) return [];
  const k = typeof stiffness === "number" ? stiffness : 0.36;
  const d = typeof damping === "number" ? damping : 0.58;
  let px = 0, py = 0, vx = 0, vy = 0;
  const out = new Array(n);
  for (let cycle = 0; cycle < 3; cycle++) {
    for (let i = 0; i < n; i++) {
      const prev = primaryDeltas[(i - 1 + n) % n];
      const cur = primaryDeltas[i];
      const next = primaryDeltas[(i + 1) % n];
      // Torso acceleration impulse drives spring lag & overshoot
      const ax = (next.dx - 2 * cur.dx + prev.dx);
      const ay = (next.dy - 2 * cur.dy + prev.dy);
      vx = (vx + (-k * px - ax * 0.45)) * d;
      vy = (vy + (-k * py - ay * 0.45)) * d;
      px += vx;
      py += vy;
      if (cycle === 2) out[i] = { dx: px, dy: py };
    }
  }
  return out;
}

function buildAutoRig(rawRgba, rawW, rawH, cleanBg, modeHint, cleanOpts) {
  const cleaned = cleanBg ? cleanRgbaPixels(rawRgba, rawW, rawH, cleanOpts) : new Uint8ClampedArray(rawRgba);
  const { grid, w, h, nativeW, nativeH, blockScale = 1 } = extractPaddedNativeGrid(cleaned, rawW, rawH);
  const defringePasses = (cleanOpts && typeof cleanOpts.defringe === "number") ? cleanOpts.defringe : 2;
  if (cleanBg && defringePasses > 0) stripExteriorGreyHalo(grid, w, h, defringePasses);
  const isHiResNative = (blockScale === 1 && Math.max(nativeW, nativeH) >= 90);
  const epx = isHiResNative ? { data: grid, w, h, factor: 1 } : Object.assign(upscale2xEpx(grid, w, h), { factor: 2 });
  const bbox = findBbox(grid, w, h);
  const inkColor = detectDarkInk(grid, w, h);

  const midX = (bbox.minX + bbox.maxX) * 0.5;
  const topCut = bbox.minY + Math.floor(bbox.bh * 0.45);
  let leftTop = 0, rightTop = 0;
  for (let y = bbox.minY; y <= topCut; y++) {
    for (let x = bbox.minX; x <= bbox.maxX; x++) {
      if (grid[(y * w + x) * 4 + 3] > 0) {
        if (x < midX) leftTop++; else rightTop++;
      }
    }
  }
  const facesLeft = leftTop >= rightTop;

  const lowerStart = bbox.minY + Math.floor(bbox.bh * 0.38);
  let lbMinX = w, lbMaxX = -1;
  for (let y = lowerStart; y <= bbox.maxY; y++) {
    for (let x = bbox.minX; x <= bbox.maxX; x++) {
      if (grid[(y * w + x) * 4 + 3] > 0) {
        if (x < lbMinX) lbMinX = x;
        if (x > lbMaxX) lbMaxX = x;
      }
    }
  }
  const lbW = Math.max(1, lbMaxX - lbMinX + 1);
  const lbH = Math.max(1, bbox.maxY - lowerStart + 1);
  const lowerAspect = lbW / lbH;

  const scanTop = bbox.minY + Math.floor(bbox.bh * 0.58);
  let maxRuns = 1, highestGapY = -1;
  for (let y = bbox.maxY; y >= scanTop; y--) {
    let runs = 0, runLen = 0;
    for (let x = bbox.minX; x <= bbox.maxX; x++) {
      if (grid[(y * w + x) * 4 + 3] > 0) { runLen++; }
      else { if (runLen >= 2) runs++; runLen = 0; }
    }
    if (runLen >= 2) runs++;
    if (runs > maxRuns) maxRuns = runs;
    if (runs >= 2) highestGapY = y;
  }

  // Belly-arch fallback: middle-third bottom sits above left/right paw bottoms
  let leftPawY = bbox.minY, rightPawY = bbox.minY, centerMinBottom = bbox.maxY;
  const q1 = bbox.minX + Math.floor(bbox.bw * 0.34);
  const q3 = bbox.minX + Math.floor(bbox.bw * 0.66);
  for (let x = bbox.minX; x <= bbox.maxX; x++) {
    let colMax = bbox.minY;
    for (let y = bbox.maxY; y >= bbox.minY; y--) {
      if (grid[(y * w + x) * 4 + 3] > 0) { colMax = y; break; }
    }
    if (x < q1 && colMax > leftPawY) leftPawY = colMax;
    else if (x > q3 && colMax > rightPawY) rightPawY = colMax;
    else if (x >= q1 && x <= q3 && colMax < centerMinBottom) centerMinBottom = colMax;
  }
  const hasBellyArch = Math.min(leftPawY, rightPawY) - centerMinBottom >= 1;

  let bodyType = modeHint && modeHint !== "auto" ? modeHint : "quadruped";
  if (!modeHint || modeHint === "auto") {
    if (maxRuns >= 3 || ((maxRuns >= 2 || hasBellyArch) && lowerAspect >= 1.12)) bodyType = "quadruped";
    else if (maxRuns >= 2 || hasBellyArch) bodyType = "biped";
    else bodyType = "blob";
  }

  const hipY = highestGapY !== -1
    ? Math.max(bbox.minY + Math.floor(bbox.bh * 0.62), highestGapY - Math.max(2, Math.round(bbox.bh * 0.06)))
    : bbox.minY + Math.floor(bbox.bh * 0.70);

  const fMin = facesLeft ? lbMinX : lbMaxX;
  const fDir = facesLeft ? 1 : -1;
  const atRelX = (t) => fMin + fDir * lbW * t;

  const headC = findColumnMassCenter(
    grid, w,
    facesLeft ? bbox.minX : bbox.maxX - bbox.bw * 0.38,
    facesLeft ? bbox.minX + bbox.bw * 0.38 : bbox.maxX,
    bbox.minY, bbox.minY + bbox.bh * 0.50,
    atRelX(0.18), bbox.minY + bbox.bh * 0.28
  );
  const chestC = findColumnMassCenter(
    grid, w,
    Math.min(atRelX(0.25), atRelX(0.55)), Math.max(atRelX(0.25), atRelX(0.55)),
    bbox.minY + bbox.bh * 0.35, hipY,
    atRelX(0.40), bbox.minY + bbox.bh * 0.52
  );
  const pelvisC = findColumnMassCenter(
    grid, w,
    Math.min(atRelX(0.55), atRelX(0.86)), Math.max(atRelX(0.55), atRelX(0.86)),
    bbox.minY + bbox.bh * 0.38, hipY,
    atRelX(0.72), bbox.minY + bbox.bh * 0.54
  );
  const tailC = findColumnMassCenter(
    grid, w,
    facesLeft ? bbox.maxX - bbox.bw * 0.14 : bbox.minX,
    facesLeft ? bbox.maxX : bbox.minX + bbox.bw * 0.14,
    bbox.minY + bbox.bh * 0.25, hipY,
    atRelX(0.95), bbox.minY + bbox.bh * 0.50
  );

  const findPawX = (t0, t1, fallbackT) => {
    const xA = Math.min(atRelX(t0), atRelX(t1));
    const xB = Math.max(atRelX(t0), atRelX(t1));
    return findColumnMassCenter(grid, w, xA, xB, Math.max(hipY, bbox.maxY - 6), bbox.maxY, atRelX(fallbackT), bbox.maxY - 1).x;
  };

  const pawY = bbox.maxY - 1;
  const pins = [
    { id: "head",   label: "Head",   color: "#ffd166", x: headC.x,   y: headC.y },
    { id: "chest",  label: "Chest",  color: "#5ec8ff", x: chestC.x,  y: chestC.y },
    { id: "pelvis", label: "Pelvis", color: "#5ec8ff", x: pelvisC.x, y: pelvisC.y },
    { id: "tail",   label: "Tail",   color: "#e07bff", x: tailC.x,   y: tailC.y },
  ];
  if (bodyType === "quadruped") {
    pins.push(
      { id: "fl", label: "FL", color: "#50f082", x: findPawX(0.08, 0.33, 0.20), y: pawY },
      { id: "fr", label: "FR", color: "#ff7373", x: findPawX(0.28, 0.52, 0.40), y: pawY },
      { id: "bl", label: "BL", color: "#ff7373", x: findPawX(0.54, 0.78, 0.67), y: pawY },
      { id: "br", label: "BR", color: "#50f082", x: findPawX(0.75, 0.98, 0.86), y: pawY }
    );
  } else if (bodyType === "biped") {
    // 2-Bone IK + opposite arm counter-swing pins (Aristidou & Lasenby 2011)
    const armY = bbox.minY + bbox.bh * 0.56;
    pins.push(
      { id: "l_hand", label: "L-Hand", color: "#ff9f43", x: atRelX(0.20), y: armY },
      { id: "r_hand", label: "R-Hand", color: "#54a0ff", x: atRelX(0.80), y: armY },
      { id: "fl", label: "L-Foot", color: "#50f082", x: findPawX(0.18, 0.50, 0.35), y: pawY },
      { id: "br", label: "R-Foot", color: "#ff7373", x: findPawX(0.50, 0.84, 0.66), y: pawY }
    );
  } else if (bodyType === "crawler") {
    // Hirose (1993) Serpenoid medial-axis spine: 5 evenly spaced pins along body length
    pins.length = 0;
    const nSpine = 5;
    for (let si = 0; si < nSpine; si++) {
      const t = 0.12 + (si / (nSpine - 1)) * 0.78;
      const sx = atRelX(t);
      const c = findColumnMassCenter(grid, w, sx - 3, sx + 3, bbox.minY, bbox.maxY, sx, bbox.minY + bbox.bh * 0.65);
      pins.push({ id: "spine_" + si, label: "S" + si, color: si === 0 ? "#ffd166" : "#50f082", x: c.x, y: c.y });
    }
  } else if (bodyType === "swimmer") {
    // Lighthill (1960) Carangiform swimmer: rigid anterior head/chest + flexible mid-tail & caudal fin
    pins.length = 0;
    const pts = [
      { id: "head", t: 0.16, col: "#ffd166" },
      { id: "chest", t: 0.38, col: "#5ec8ff" },
      { id: "pelvis", t: 0.64, col: "#50f082" },
      { id: "tail", t: 0.90, col: "#e07bff" },
    ];
    for (let i = 0; i < pts.length; i++) {
      const sx = atRelX(pts[i].t);
      const c = findColumnMassCenter(grid, w, sx - 3, sx + 3, bbox.minY, bbox.maxY, sx, bbox.minY + bbox.bh * 0.5);
      pins.push({ id: pts[i].id, label: pts[i].id, color: pts[i].col, x: c.x, y: c.y });
    }
  } else if (bodyType === "bird") {
    // Tobalske (2007) Asymmetric flapping-wing kinematics: wingtip pin + tail pitch + feet
    const wingC = findColumnMassCenter(
      grid, w,
      Math.min(atRelX(0.32), atRelX(0.72)), Math.max(atRelX(0.32), atRelX(0.72)),
      bbox.minY + bbox.bh * 0.22, bbox.minY + bbox.bh * 0.62,
      atRelX(0.52), bbox.minY + bbox.bh * 0.42
    );
    pins.push(
      { id: "wing", label: "Wing", color: "#ff9f43", x: wingC.x, y: wingC.y },
      { id: "fl", label: "Foot", color: "#50f082", x: findPawX(0.30, 0.70, 0.50), y: pawY }
    );
  } else {
    pins.push(
      { id: "fl", label: "BaseL", color: "#50f082", x: atRelX(0.28), y: pawY },
      { id: "br", label: "BaseR", color: "#50f082", x: atRelX(0.72), y: pawY }
    );
  }

  // Refine pins using true Medial-Axis skeleton core & limb endpoints (Zhang-Suen + Distance Transform)
  const medial = extractMedialAxisFeatures(grid, w, h, bbox);
  if (medial.endpoints.length > 0 && bodyType !== "blob") {
    const snapRadius = Math.max(8, Math.min(bbox.bw, bbox.bh) * 0.24);
    for (let pi = 0; pi < pins.length; pi++) {
      const pin = pins[pi];
      if (pin.id === "chest") {
        // Blend chest gently toward true distance-transform torso core
        pin.x = pin.x * 0.65 + medial.core.x * 0.35;
        pin.y = pin.y * 0.65 + medial.core.y * 0.35;
        continue;
      }
      if (pin.id === "pelvis") continue;
      let bestEp = null, bestD = snapRadius;
      for (let ei = 0; ei < medial.endpoints.length; ei++) {
        const ep = medial.endpoints[ei];
        // Keep paw pins in lower half and head/tail pins out of the floor
        if ((pin.id === "fl" || pin.id === "fr" || pin.id === "bl" || pin.id === "br") && ep.y < hipY) continue;
        if ((pin.id === "head" || pin.id === "tail") && ep.y > bbox.maxY - Math.max(3, bbox.bh * 0.14)) continue;
        const d = Math.hypot(ep.x - pin.x, ep.y - pin.y);
        if (d < bestD) { bestD = d; bestEp = ep; }
      }
      if (bestEp) {
        // Blend 65% toward real skeleton extremity so paws/ears/tails lock onto actual silhouette tips
        pin.x = pin.x * 0.35 + bestEp.x * 0.65;
        pin.y = pin.y * 0.35 + bestEp.y * 0.65;
      }
    }
  }

  const geo = computeGeodesicSkinning(grid, w, h, pins);
  return { grid, epx, w, h, nativeW, nativeH, isHiResNative, bbox, hipY, facesLeft, bodyType, maxRuns, pins, inkColor, geo, medial };
}

// FIXED FORWARD GAIT PHYSICS:
// When facesLeft = true (dir = -1):
//   Swing phase (lifted: cos(phase) > 0, phase in [-pi/2, +pi/2]): dx = dir * sin(phase) moves +stride -> -stride (FORWARD LEFT!)
//   Stance phase (planted: cos(phase) < 0, phase in [pi/2, 3pi/2]): dx moves -stride -> +stride (pushes ground BACKWARD!)
function computeAnimatedPins(rig, fx, frameIdx, totalFrames, ampScaleOpt) {
  const { bbox, facesLeft, bodyType, pins } = rig;
  let ampScale = typeof ampScaleOpt === "number" ? ampScaleOpt : 1.0;
  if (typeof ampScaleOpt !== "number" && typeof document !== "undefined") {
    const ampEl = document.getElementById("gif-amp");
    if (ampEl) ampScale = Math.max(0.3, Math.min(2.0, parseFloat(ampEl.value) || 1.0));
  }
  const phase = (frameIdx / totalFrames) * Math.PI * 2;
  const dir = facesLeft ? -1 : 1;
  // Restrained stride prevents opposite-moving paws from stretching outlines across empty gaps
  const stride = Math.max(1.8, bbox.bw * 0.062);
  const liftMax = Math.max(1.6, (bbox.maxY - rig.hipY) * 0.22);

  return pins.map((pin) => {
    let dx = 0, dy = 0;
    if (fx === "walk") {
      if (bodyType === "crawler" && pin.id.startsWith("spine_")) {
        // Hirose (1993) Serpenoid traveling wave: head stabilized, trailing wave along spine
        const si = parseInt(pin.id.slice(6), 10) || 0;
        const amp = si === 0 ? 0.45 : Math.max(1.2, bbox.bh * 0.14) * (0.45 + 0.55 * (si / 4));
        dy = -Math.abs(Math.sin(phase - si * 1.05)) * amp;
        dx = dir * Math.cos(phase - si * 1.05) * (si === 0 ? 0.5 : 1.1);
      } else if (bodyType === "swimmer") {
        // Lighthill (1960) Carangiform swimming: rigid head, quadratic caudal fin wave + buoyancy
        const buoy = Math.sin(phase) * Math.max(0.8, bbox.bh * 0.04);
        if (pin.id === "head") { dy = buoy * 0.5; }
        else if (pin.id === "chest") { dy = buoy * 0.8 + Math.sin(phase - 0.5) * 0.5; }
        else if (pin.id === "pelvis") { dy = buoy + Math.sin(phase - 1.2) * Math.max(1.4, bbox.bh * 0.07); }
        else if (pin.id === "tail") {
          dy = buoy + Math.sin(phase - 2.1) * Math.max(2.6, bbox.bh * 0.16);
          dx = -dir * Math.abs(Math.cos(phase - 2.1)) * 1.1;
        }
      } else if (bodyType === "bird") {
        // Tobalske (2007) Asymmetric flapping: fast downstroke lifts body, tucked upstroke dips body
        const flap = Math.sin(phase);
        const bodyLift = -flap * Math.max(1.4, bbox.bh * 0.06);
        if (pin.id === "wing") {
          dy = flap * Math.max(3.2, bbox.bh * 0.22);
          dx = -dir * Math.cos(phase) * 1.2;
        } else if (pin.id === "tail") {
          dy = bodyLift - flap * 1.4;
        } else {
          dy = bodyLift;
        }
      } else {
        const sA = Math.sin(phase);
        const sB = Math.sin(phase + Math.PI);
        const lA = Math.max(0, Math.cos(phase));
        const lB = Math.max(0, Math.cos(phase + Math.PI));
        const bob = -Math.abs(Math.cos(phase)) * Math.max(0.8, bbox.bh * 0.028);
        if (pin.id === "head") {
          dy = -Math.abs(Math.cos(phase - 0.8)) * Math.max(1.0, bbox.bh * 0.036);
          dx = dir * Math.sin(phase - 0.6) * 0.9;
        } else if (pin.id === "chest") {
          dy = bob;
        } else if (pin.id === "pelvis") {
          dy = bob * 0.9;
        } else if (pin.id === "tail") {
          dy = bob + Math.sin(phase * 2) * 1.4;
          dx = -dir * Math.cos(phase) * 0.7;
        } else if (bodyType === "biped" && pin.id === "l_hand") {
          // Opposite-phase arm counter-swing (L-Hand swings opposite to L-Foot `fl`)
          dx = dir * sB * stride * 0.85;
          dy = bob - Math.abs(sB) * 0.8;
        } else if (bodyType === "biped" && pin.id === "r_hand") {
          dx = dir * sA * stride * 0.85;
          dy = bob - Math.abs(sA) * 0.8;
        } else if (bodyType === "biped" && pin.id === "fl") {
          dx = dir * sA * stride;
          dy = -lA * liftMax;
        } else if (bodyType === "biped" && pin.id === "br") {
          dx = dir * sB * stride;
          dy = -lB * liftMax;
        } else if (bodyType !== "blob" && (pin.id === "fl" || pin.id === "br")) {
          dx = dir * sA * stride;
          dy = -lA * liftMax;
        } else if (bodyType !== "blob" && (pin.id === "fr" || pin.id === "bl")) {
          dx = dir * sB * stride;
          dy = -lB * liftMax;
        }
      }
    } else if (fx === "run") {
      const runS = Math.max(2.4, bbox.bw * 0.088);
      const runL = Math.max(2.2, (bbox.maxY - rig.hipY) * 0.30);
      const airBob = -Math.max(0, Math.sin(phase)) * Math.max(1.8, bbox.bh * 0.06);
      if (pin.id === "head") { dx = dir * 1.4; dy = airBob; }
      else if (pin.id === "chest" || pin.id === "pelvis") { dy = airBob; dx = dir * 0.5; }
      else if (pin.id === "tail") { dy = airBob + Math.sin(phase * 2) * 1.8; dx = -dir * 1.0; }
      else if (pin.id === "fl") { dx = dir * Math.sin(phase) * runS; dy = -Math.max(0, Math.cos(phase)) * runL; }
      else if (pin.id === "fr") { dx = dir * Math.sin(phase + 0.55) * runS; dy = -Math.max(0, Math.cos(phase + 0.55)) * runL; }
      else if (pin.id === "bl") { dx = dir * Math.sin(phase + Math.PI * 0.85) * runS; dy = -Math.max(0, Math.cos(phase + Math.PI * 0.85)) * runL; }
      else if (pin.id === "br") { dx = dir * Math.sin(phase + Math.PI * 1.15) * runS; dy = -Math.max(0, Math.cos(phase + Math.PI * 1.15)) * runL; }
    } else if (fx === "sprint") {
      // High-energy gallop/sprint: 1.45x stride, forward torso pitch, airborne suspension phase
      const sA = Math.sin(phase);
      const sB = Math.sin(phase + Math.PI * 0.72);
      const lA = Math.max(0, Math.cos(phase));
      const lB = Math.max(0, Math.cos(phase + Math.PI * 0.72));
      const airLift = -Math.max(0, Math.sin(phase * 2)) * Math.max(1.8, bbox.bh * 0.065);
      if (pin.id === "head") { dx = dir * 1.8; dy = airLift - 0.6; }
      else if (pin.id === "chest") { dx = dir * 1.1; dy = airLift; }
      else if (pin.id === "pelvis") { dy = airLift * 0.85; }
      else if (pin.id === "tail") { dx = -dir * (0.4 + Math.cos(phase) * 0.6); dy = airLift + Math.sin(phase * 2) * 1.4; }
      else if (pin.id === "fl" || pin.id === "fr") { dx = dir * sA * stride * 1.45; dy = airLift - lA * liftMax * 1.4; }
      else if (pin.id === "bl" || pin.id === "br") { dx = dir * sB * stride * 1.45; dy = airLift - lB * liftMax * 1.4; }
      else { dy = airLift; }
    } else if (fx === "dance") {
      // Rhythmic groove: alternating sway, double-time head bob, paw taps
      const sway = Math.sin(phase) * Math.max(1.6, bbox.bw * 0.045);
      const bounce = -Math.abs(Math.sin(phase * 2)) * Math.max(1.4, bbox.bh * 0.05);
      if (pin.id === "head") { dx = sway * 1.3; dy = bounce * 1.35; }
      else if (pin.id === "chest") { dx = sway; dy = bounce; }
      else if (pin.id === "pelvis") { dx = sway * 0.35; dy = bounce * 0.6; }
      else if (pin.id === "tail") { dx = sway * 0.15; dy = bounce + Math.sin(phase * 2) * 1.4; }
      else if (pin.id === "fl" || pin.id === "br") { dy = -Math.max(0, Math.sin(phase)) * liftMax * 0.8; }
      else if (pin.id === "fr" || pin.id === "bl") { dy = -Math.max(0, -Math.sin(phase)) * liftMax * 0.8; }
    } else if (fx === "pet") {
      // Happy head-pat squish + rapid tail wag
      const squish = Math.max(0, Math.sin(phase));
      if (pin.id === "head") { dy = squish * Math.max(2.2, bbox.bh * 0.075); dx = dir * squish * 0.6; }
      else if (pin.id === "chest") { dy = squish * Math.max(1.2, bbox.bh * 0.04); }
      else if (pin.id === "tail") { dy = Math.sin(phase * 4) * Math.max(2.0, bbox.bh * 0.07); dx = -dir * Math.cos(phase * 4) * 1.0; }
    } else if (fx === "sleep") {
      // Real crouched nap posture: lower head/torso toward ground + slow deep breathing swell
      const br = Math.sin(phase);
      const crouch = Math.max(4.0, bbox.bh * 0.11);
      if (pin.id === "head") { dy = crouch * 1.25 + br * 1.2; dx = dir * 1.8; }
      else if (pin.id === "chest") { dy = crouch * 0.85 - br * Math.max(1.6, bbox.bh * 0.042); }
      else if (pin.id === "pelvis") { dy = crouch * 0.70 - br * Math.max(1.0, bbox.bh * 0.026); }
      else if (pin.id === "tail") { dy = crouch * 0.65 + br * 1.2; dx = -dir * 1.0; }
    } else if (fx === "breathe") {
      const ex = (Math.sin(phase) * 0.5 + 0.5) * Math.max(1.0, bbox.bh * 0.04);
      if (pin.id === "head" || pin.id === "chest") dy = ex;
      else if (pin.id === "pelvis") dy = ex * 0.65;
      else if (pin.id === "tail") dy = ex * 0.8;
    } else if (fx === "hop") {
      const u = frameIdx / totalFrames;
      if (u < 0.24) {
        if (pin.id === "head") dy = bbox.bh * 0.045;
        else if (pin.id === "chest" || pin.id === "pelvis" || pin.id === "tail") dy = bbox.bh * 0.03;
      } else if (u < 0.68) {
        const air = Math.sin(((u - 0.24) / 0.44) * Math.PI);
        const hLift = air * Math.max(4.0, bbox.bh * 0.17);
        dy = -hLift;
        if (pin.id === "head") dy -= air * 1.0;
        if (pin.id === "fl" || pin.id === "fr") { dx = dir * air * 1.4; dy += air * 1.0; }
        if (pin.id === "bl" || pin.id === "br") { dx = -dir * air * 1.4; dy += air * 1.0; }
      } else {
        if (pin.id === "head") dy = bbox.bh * 0.03;
        else if (pin.id === "chest" || pin.id === "pelvis") dy = bbox.bh * 0.02;
      }
    } else if (fx === "wave") {
      if (pin.id === "tail") { dy = Math.sin(phase) * 2.2; dx = dir * Math.cos(phase) * 1.2; }
      else if (pin.id === "head") { dy = Math.sin(phase + 1.1) * 1.0; }
    }
    return { id: pin.id, x: pin.x + dx * ampScale, y: pin.y + dy * ampScale };
  });
}

function computeAnimatedPinsWithSpring(rig, fx, frameIdx, totalFrames, ampScaleOpt) {
  const base = computeAnimatedPins(rig, fx, frameIdx, totalFrames, ampScaleOpt);
  if (fx === "breathe" || fx === "sleep" || totalFrames < 3) return base;
  // Compute periodic torso motion curve across the cycle for secondary spring follow-through on tail/head/wing
  const torsoIdx = rig.pins.findIndex((p) => p.id === "chest" || p.id === "pelvis");
  if (torsoIdx === -1) return base;
  const primaryDeltas = [];
  for (let fi = 0; fi < totalFrames; fi++) {
    const fp = computeAnimatedPins(rig, fx, fi, totalFrames, ampScaleOpt);
    primaryDeltas.push({
      dx: fp[torsoIdx].x - rig.pins[torsoIdx].x,
      dy: fp[torsoIdx].y - rig.pins[torsoIdx].y,
    });
  }
  const springTail = computeSpringFollowThrough(primaryDeltas, 0.34, 0.62);
  const springHead = computeSpringFollowThrough(primaryDeltas, 0.46, 0.52);
  const fi = ((frameIdx % totalFrames) + totalFrames) % totalFrames;
  const st = springTail[fi] || { dx: 0, dy: 0 };
  const sh = springHead[fi] || { dx: 0, dy: 0 };
  return base.map((p) => {
    if (p.id === "tail") return { id: p.id, x: p.x + st.dx * 0.9, y: p.y + st.dy * 1.15 };
    if (p.id === "head") return { id: p.id, x: p.x + sh.dx * 0.35, y: p.y + sh.dy * 0.45 };
    if (p.id === "wing") return { id: p.id, x: p.x + st.dx * 0.5, y: p.y + st.dy * 0.65 };
    return p;
  });
}


// Precompute 8-connected Interior Geodesic Weights (Jacobson 2011 / Schaefer 2006)
// Stepping through opaque silhouette costs 1.0 (or 1.414 diag); crossing transparent air costs 12.0x!
// This mathematically isolates BL/BR and FL/FR legs across transparent gaps (zero paw stretching).
function computeGeodesicSkinning(grid, w, h, pins) {
  const n = w * h;
  const numPins = pins.length;
  const weights = new Float32Array(n * numPins);
  const domPin = new Uint8Array(n);
  const dists = new Float32Array(n * numPins);

  // Simple binary min-heap for fast Dijkstra on native pixel grid (< 2ms total)
  const heapIdx = new Int32Array(n * 8);
  const heapDist = new Float32Array(n * 8);

  const dx8 = [-1, 1, 0, 0, -1, 1, -1, 1];
  const dy8 = [0, 0, -1, 1, -1, -1, 1, 1];
  const dc8 = [1.0, 1.0, 1.0, 1.0, 1.4142, 1.4142, 1.4142, 1.4142];

  for (let pi = 0; pi < numPins; pi++) {
    const base = pi * n;
    for (let i = 0; i < n; i++) dists[base + i] = 1e9;
    const sx = Math.max(0, Math.min(w - 1, Math.round(pins[pi].x)));
    const sy = Math.max(0, Math.min(h - 1, Math.round(pins[pi].y)));
    const startIdx = sy * w + sx;
    dists[base + startIdx] = 0;
    let hSize = 0;
    const push = (idx, d) => {
      let cur = hSize++;
      heapIdx[cur] = idx;
      heapDist[cur] = d;
      while (cur > 0) {
        const par = (cur - 1) >> 1;
        if (heapDist[par] <= heapDist[cur]) break;
        const ti = heapIdx[par], td = heapDist[par];
        heapIdx[par] = heapIdx[cur]; heapDist[par] = heapDist[cur];
        heapIdx[cur] = ti; heapDist[cur] = td;
        cur = par;
      }
    };
    const pop = () => {
      const top = heapIdx[0];
      hSize--;
      if (hSize > 0) {
        heapIdx[0] = heapIdx[hSize];
        heapDist[0] = heapDist[hSize];
        let cur = 0;
        while (true) {
          const l = cur * 2 + 1, r = l + 1;
          let sm = cur;
          if (l < hSize && heapDist[l] < heapDist[sm]) sm = l;
          if (r < hSize && heapDist[r] < heapDist[sm]) sm = r;
          if (sm === cur) break;
          const ti = heapIdx[cur], td = heapDist[cur];
          heapIdx[cur] = heapIdx[sm]; heapDist[cur] = heapDist[sm];
          heapIdx[sm] = ti; heapDist[sm] = td;
          cur = sm;
        }
      }
      return top;
    };

    push(startIdx, 0);
    while (hSize > 0) {
      const topD = heapDist[0];
      const u = pop();
      const du = dists[base + u];
      if (topD > du + 1e-5) continue;
      const ux = u % w, uy = (u / w) | 0;
      for (let k = 0; k < 8; k++) {
        const nx = ux + dx8[k], ny = uy + dy8[k];
        if (nx < 0 || nx >= w || ny < 0 || ny >= h) continue;
        const v = ny * w + nx;
        const isOpaque = grid[v * 4 + 3] > 0;
        const stepCost = dc8[k] * (isOpaque ? 1.0 : 12.0);
        const nd = du + stepCost;
        if (nd < dists[base + v]) {
          dists[base + v] = nd;
          push(v, nd);
        }
      }
    }
  }

  for (let i = 0; i < n; i++) {
    let wSum = 0, bestW = -1, bestP = 0;
    for (let pi = 0; pi < numPins; pi++) {
      const d = dists[pi * n + i];
      const wi = 1.0 / Math.pow(d + 1.25, 3.5);
      weights[i * numPins + pi] = wi;
      wSum += wi;
      if (wi > bestW) { bestW = wi; bestP = pi; }
    }
    const inv = wSum > 0 ? (1.0 / wSum) : (1.0 / numPins);
    for (let pi = 0; pi < numPins; pi++) weights[i * numPins + pi] *= inv;
    domPin[i] = bestP;
  }

  return { weights, domPin, numPins };
}

function evalRigidMlsForward(vx, vy, srcPins, dstPins, wRow, wOffset) {
  const n = srcPins.length;
  let pxStar = 0, pyStar = 0, qxStar = 0, qyStar = 0;
  for (let i = 0; i < n; i++) {
    const wi = wRow[wOffset + i];
    pxStar += wi * srcPins[i].x; pyStar += wi * srcPins[i].y;
    qxStar += wi * dstPins[i].x; qyStar += wi * dstPins[i].y;
  }
  const dvx = vx - pxStar, dvy = vy - pyStar;
  const dvLen = Math.hypot(dvx, dvy);
  if (dvLen < 1e-6) return { x: qxStar, y: qyStar };

  let frx = 0, fry = 0;
  for (let i = 0; i < n; i++) {
    const wi = wRow[wOffset + i];
    const phx = srcPins[i].x - pxStar, phy = srcPins[i].y - pyStar;
    const qhx = dstPins[i].x - qxStar, qhy = dstPins[i].y - qyStar;
    const dot = phx * dvx + phy * dvy;
    const cross = phx * dvy - phy * dvx;
    frx += wi * (qhx * dot - qhy * cross);
    fry += wi * (qhy * dot + qhx * cross);
  }
  const frLen = Math.hypot(frx, fry);
  if (frLen < 1e-6) return { x: qxStar, y: qyStar };
  const scale = dvLen / frLen;
  return { x: qxStar + frx * scale, y: qyStar + fry * scale };
}

function rasterizeTriEpx(out, zBuf, w, h, epxData, ew, eh, factor, ax, ay, au, av, bx, by, bu, bv, cx, cy, cu, cv, depth) {
  const minX = Math.max(0, Math.floor(Math.min(ax, bx, cx)));
  const maxX = Math.min(w - 1, Math.ceil(Math.max(ax, bx, cx)));
  const minY = Math.max(0, Math.floor(Math.min(ay, by, cy)));
  const maxY = Math.min(h - 1, Math.ceil(Math.max(ay, by, cy)));
  const denom = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy);
  if (Math.abs(denom) < 1e-5) return;
  const invD = 1.0 / denom;

  for (let py = minY; py <= maxY; py++) {
    const cyP = py + 0.5;
    for (let px = minX; px <= maxX; px++) {
      const cxP = px + 0.5;
      const l1 = ((by - cy) * (cxP - cx) + (cx - bx) * (cyP - cy)) * invD;
      const l2 = ((cy - ay) * (cxP - cx) + (ax - cx) * (cyP - cy)) * invD;
      const l3 = 1.0 - l1 - l2;
      if (l1 < -0.02 || l2 < -0.02 || l3 < -0.02) continue;
      const dstIdx = py * w + px;
      if (depth < zBuf[dstIdx]) continue;
      const u = l1 * au + l2 * bu + l3 * cu;
      const v = l1 * av + l2 * bv + l3 * cv;
      const ex = Math.max(0, Math.min(ew - 1, Math.round(u * factor)));
      const ey = Math.max(0, Math.min(eh - 1, Math.round(v * factor)));
      const sp = (ey * ew + ex) * 4;
      if (epxData[sp + 3] === 0) continue;
      const dp = dstIdx * 4;
      out[dp] = epxData[sp]; out[dp + 1] = epxData[sp + 1]; out[dp + 2] = epxData[sp + 2]; out[dp + 3] = 255;
      zBuf[dstIdx] = depth;
    }
  }
}

function evalRigidMlsInverse(vx, vy, srcPins, dstPins) {
  const n = dstPins.length;
  let wSum = 0, qxStar = 0, qyStar = 0, pxStar = 0, pyStar = 0;
  const wArr = new Float64Array(n);

  for (let i = 0; i < n; i++) {
    const dx = vx - dstPins[i].x, dy = vy - dstPins[i].y;
    const dist2 = dx * dx + dy * dy;
    if (dist2 < 1e-6) return { x: srcPins[i].x, y: srcPins[i].y };
    const wi = 1.0 / Math.pow(dist2, 1.65);
    wArr[i] = wi; wSum += wi;
    qxStar += wi * dstPins[i].x; qyStar += wi * dstPins[i].y;
    pxStar += wi * srcPins[i].x; pyStar += wi * srcPins[i].y;
  }
  qxStar /= wSum; qyStar /= wSum; pxStar /= wSum; pyStar /= wSum;

  const dvx = vx - qxStar, dvy = vy - qyStar;
  const dvLen = Math.hypot(dvx, dvy);
  if (dvLen < 1e-6) return { x: pxStar, y: pyStar };

  let frx = 0, fry = 0;
  for (let i = 0; i < n; i++) {
    const qhx = dstPins[i].x - qxStar, qhy = dstPins[i].y - qyStar;
    const phx = srcPins[i].x - pxStar, phy = srcPins[i].y - pyStar;
    const wi = wArr[i];
    const dot = qhx * dvx + qhy * dvy;
    const cross = qhx * dvy - qhy * dvx;
    frx += wi * (phx * dot - phy * cross);
    fry += wi * (phy * dot + phx * cross);
  }
  const frLen = Math.hypot(frx, fry);
  if (frLen < 1e-6) return { x: pxStar, y: pyStar };
  const scale = dvLen / frLen;
  return { x: pxStar + frx * scale, y: pyStar + fry * scale };
}

// Geodesic-Weighted Rigid MLS + Depth-Sorted 2x EPX Quad Rasterizer
// Zero rubber stretching across leg gaps, zero 1px seam holes, true far/near leg depth ordering!
// 3-Pass Layered Articulated Limb + Geodesic Torso Renderer
// Solves opposing-leg MLS cancellation on thick-legged sprites:
//   Pass 1 (Far Limbs: fr, bl) -> Pass 2 (Body: torso/head/tail MLS) -> Pass 3 (Near Limbs: fl, br)
// Each limb rotates cleanly around its shoulder/hip pivot with zero cross-leg rubber stretching.
// Watertight Torso-MLS + Non-Colliding Pair-Gap Articulated Limb Rig
// 1. Torso MLS uses ONLY non-limb pins (head, chest, pelvis, tail) so legs never distort the belly.
// 2. Each limb vertex blends from exact torso position at y <= hipAnchorY (s = 0) via smoothstep(t),
//    making shoulder/groin seams mathematically watertight (zero tears).
// 3. Inward leg swing is clamped against the real rest-pose pair gap (|fr.x - fl.x|, |br.x - bl.x|)
//    with passing-phase vertical clearance so paired paws never crash into a single clump.
// Rigid-Bone Articulated Limb + Watertight Torso-MLS Renderer
// Eliminates banana-shearing and melted paws:
// - Pure constant-angle rigid rotation below a 4px hip-socket blend zone (zero shiftX stretching!)
// - Pair-gap angle clamping so paired paws never collide
// - Planted feet on Dance, clean airborne tuck on Hop, real crouched nap on Sleep
function computeLimbGaitPose(rig, fx, frameIdx, totalFrames, ampScale) {
  const phase = (frameIdx / Math.max(1, totalFrames)) * Math.PI * 2;
  const dir = rig.facesLeft ? -1 : 1;
  const legH = Math.max(8, rig.bbox.maxY - rig.hipY);
  const byId = {};
  for (let i = 0; i < rig.pins.length; i++) byId[rig.pins[i].id] = rig.pins[i];

  const frontGap = (byId.fl && byId.fr) ? Math.abs(byId.fr.x - byId.fl.x) : rig.bbox.bw * 0.22;
  const backGap = (byId.bl && byId.br) ? Math.abs(byId.br.x - byId.bl.x) : rig.bbox.bw * 0.22;

  const clampPairAngle = (id, rawAngle) => {
    const isFront = (id === "fl" || id === "fr");
    const partner = id === "fl" ? byId.fr : (id === "fr" ? byId.fl : (id === "bl" ? byId.br : byId.bl));
    const self = byId[id];
    if (!self || !partner) return rawAngle;
    const gap = Math.max(4, isFront ? frontGap : backGap);
    const towardPartner = Math.sign(partner.x - self.x) || 1;
    const tipDx = -Math.sin(rawAngle) * legH;
    if (tipDx * towardPartner > 0) {
      const maxInward = gap * 0.34;
      if (Math.abs(tipDx) > maxInward) {
        return rawAngle * (maxInward / Math.abs(tipDx));
      }
    }
    return rawAngle;
  };

  const poses = {};
  if (fx === "walk") {
    const pairs = [["fl", 0], ["br", 0], ["fr", Math.PI], ["bl", Math.PI]];
    for (const [id, off] of pairs) {
      const p = phase + off;
      const swing = Math.sin(p);
      const lift = Math.max(0, Math.cos(p));
      poses[id] = {
        angle: clampPairAngle(id, -dir * swing * 0.22 * ampScale),
        liftY: -lift * Math.max(1.8, legH * 0.16) * ampScale,
        shiftX: 0,
      };
    }
  } else if (fx === "sprint" || fx === "run") {
    const specs = [["fl", 0.0], ["fr", 0.50], ["bl", Math.PI * 0.90], ["br", Math.PI * 1.25]];
    for (const [id, off] of specs) {
      const p = phase + off;
      const swing = Math.sin(p);
      const lift = Math.max(0, Math.cos(p));
      poses[id] = {
        angle: clampPairAngle(id, -dir * swing * 0.34 * ampScale),
        liftY: -lift * Math.max(2.6, legH * 0.24) * ampScale,
        shiftX: 0,
      };
    }
  } else if (fx === "dance") {
    // Keep paws planted cleanly on the beat with subtle alternating lift (no ice-skating slide)
    const specs = [["fl", 0], ["br", 0], ["fr", Math.PI], ["bl", Math.PI]];
    for (const [id, off] of specs) {
      const p = phase + off;
      const tap = Math.max(0, Math.sin(p));
      poses[id] = {
        angle: clampPairAngle(id, dir * Math.sin(p) * 0.08 * ampScale),
        liftY: -tap * Math.max(1.8, legH * 0.18) * ampScale,
        shiftX: 0,
      };
    }
  } else if (fx === "hop") {
    const u = frameIdx / Math.max(1, totalFrames);
    const air = (u >= 0.24 && u < 0.68) ? Math.sin(((u - 0.24) / 0.44) * Math.PI) : 0;
    const tuck = air * 0.26 * ampScale;
    for (const id of ["fl", "fr", "bl", "br"]) {
      const isFront = (id === "fl" || id === "fr");
      poses[id] = {
        angle: (isFront ? -dir : dir) * tuck,
        liftY: 0,
        shiftX: 0,
      };
    }
  }
  return poses;
}


// Seal 1px barycentric triangle-seam pinholes (surrounded on all 4 cardinal sides)
// and remove tiny 1-2px detached outline flecks from rotated paws.
function sealPinholesAndSpecks(out, w, h) {
  for (let pass = 0; pass < 2; pass++) {
    const fills = [];
    for (let y = 1; y < h - 1; y++) {
      for (let x = 1; x < w - 1; x++) {
        const i = y * w + x;
        if (out[i * 4 + 3] > 0) continue;
        const iL = i - 1, iR = i + 1, iU = i - w, iD = i + w;
        if (out[iL * 4 + 3] > 0 && out[iR * 4 + 3] > 0 && out[iU * 4 + 3] > 0 && out[iD * 4 + 3] > 0) {
          const r = (out[iL * 4] + out[iR * 4] + out[iU * 4] + out[iD * 4]) >> 2;
          const g = (out[iL * 4 + 1] + out[iR * 4 + 1] + out[iU * 4 + 1] + out[iD * 4 + 1]) >> 2;
          const b = (out[iL * 4 + 2] + out[iR * 4 + 2] + out[iU * 4 + 2] + out[iD * 4 + 2]) >> 2;
          fills.push(i, r, g, b);
        }
      }
    }
    for (let k = 0; k < fills.length; k += 4) {
      const dp = fills[k] * 4;
      out[dp] = fills[k + 1]; out[dp + 1] = fills[k + 2]; out[dp + 2] = fills[k + 3]; out[dp + 3] = 255;
    }
    if (fills.length === 0) break;
  }
  const vis = new Uint8Array(w * h);
  const q = new Int32Array(w * h);
  for (let i = 0; i < w * h; i++) {
    if (vis[i] || out[i * 4 + 3] === 0) continue;
    let head = 0, tail = 0;
    vis[i] = 1; q[tail++] = i;
    while (head < tail) {
      const u = q[head++], ux = u % w, uy = (u / w) | 0;
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          const nx = ux + dx, ny = uy + dy;
          if (nx < 0 || nx >= w || ny < 0 || ny >= h) continue;
          const v = ny * w + nx;
          if (!vis[v] && out[v * 4 + 3] > 0) { vis[v] = 1; q[tail++] = v; }
        }
      }
    }
    if (tail <= 2) {
      for (let k = 0; k < tail; k++) {
        const dp = q[k] * 4;
        out[dp] = 0; out[dp + 1] = 0; out[dp + 2] = 0; out[dp + 3] = 0;
      }
    }
  }
}


// ============================================================================
// UNIFIED HIERARCHICAL BONE-FK ENGINE (All 6 Archetypes + Custom User Pins)
// ============================================================================
// Replaces hardcoded hipAnchorY cuts with a true parent->child skeleton graph:
//   - Parent transforms carry children rigidly (zero detachment on any sprite)
//   - Local joint rotation pivots cleanly around the parent->child socket
//   - 5px socket blend + 1-step overlap collar keeps every joint watertight
function buildBoneHierarchy(pins, bodyType) {
  const byId = new Map();
  for (let i = 0; i < pins.length; i++) byId.set(pins[i].id, i);

  const parentOf = new Int16Array(pins.length).fill(-1);
  const depthOf = new Int8Array(pins.length).fill(3);

  const setParent = (childId, parentId) => {
    const ci = byId.get(childId), pi = byId.get(parentId);
    if (ci !== undefined && pi !== undefined && ci !== pi) parentOf[ci] = pi;
  };

  if (bodyType === "crawler") {
    for (let i = 1; i < pins.length; i++) {
      parentOf[i] = i - 1;
      depthOf[i] = 3;
    }
    if (pins.length > 0) depthOf[0] = 4;
  } else if (bodyType === "swimmer") {
    setParent("head", "chest");
    setParent("pelvis", "chest");
    setParent("tail", "pelvis");
    for (let i = 0; i < pins.length; i++) {
      const id = pins[i].id;
      depthOf[i] = id === "head" ? 4 : (id === "tail" ? 2 : 3);
    }
  } else {
    // Quadruped, Biped, Bird, Blob: chest is root
    const rootIdx = byId.has("chest") ? byId.get("chest") : 0;
    setParent("head", "chest");
    setParent("pelvis", "chest");
    setParent("tail", byId.has("pelvis") ? "pelvis" : "chest");
    setParent("fl", "chest");
    setParent("fr", "chest");
    setParent("l_hand", "chest");
    setParent("r_hand", "chest");
    setParent("wing", "chest");
    setParent("bl", byId.has("pelvis") ? "pelvis" : "chest");
    setParent("br", byId.has("pelvis") ? "pelvis" : "chest");

    // Auto-parent any custom/unknown pins to nearest core bone
    for (let i = 0; i < pins.length; i++) {
      if (i === rootIdx || parentOf[i] !== -1) continue;
      parentOf[i] = rootIdx;
    }

    for (let i = 0; i < pins.length; i++) {
      const id = pins[i].id;
      if (id === "fr" || id === "bl" || id === "r_hand") depthOf[i] = 1;
      else if (id === "tail") depthOf[i] = 2;
      else if (id === "chest" || id === "pelvis") depthOf[i] = 3;
      else if (id === "head") depthOf[i] = 4;
      else if (id === "fl" || id === "br") depthOf[i] = 5;
      else if (id === "wing" || id === "l_hand") depthOf[i] = 6;
      else depthOf[i] = 5;
    }
  }

  // Topological order (parents always evaluated before children)
  const order = [];
  const visited = new Uint8Array(pins.length);
  const visit = (i) => {
    if (visited[i]) return;
    if (parentOf[i] !== -1) visit(parentOf[i]);
    visited[i] = 1;
    order.push(i);
  };
  for (let i = 0; i < pins.length; i++) visit(i);

  // Compute anatomical joint socket pivot (between parent and child, biased toward parent)
  const sockets = new Array(pins.length);
  for (let i = 0; i < pins.length; i++) {
    const p = pins[i];
    const pi = parentOf[i];
    if (pi === -1) {
      sockets[i] = { x: p.x, y: p.y, len: 1 };
    } else {
      const par = pins[pi];
      const id = p.id;
      const isLimb = (id === "fl" || id === "fr" || id === "bl" || id === "br");
      // For legs, place hip/shoulder socket cleanly below torso center; for others, 35% from parent
      const sx = isLimb ? (p.x * 0.78 + par.x * 0.22) : (par.x * 0.65 + p.x * 0.35);
      const sy = isLimb ? (par.y * 0.68 + p.y * 0.32) : (par.y * 0.65 + p.y * 0.35);
      sockets[i] = { x: sx, y: sy, len: Math.max(4, Math.hypot(p.x - sx, p.y - sy)) };
    }
  }

  return { parentOf, depthOf, order, sockets };
}

function computeBoneFkTransforms(rig, fx, frameIdx, totalFrames, ampScale) {
  const _mL = rig.bbox.minX, _mR = rig.w - 1 - rig.bbox.maxX, _mT = rig.bbox.minY;
  const _minMargin = Math.min(_mL, _mR, _mT);
  if (rig.bodyType === "crawler" && _minMargin < 12) {
    ampScale = (typeof ampScale === "number" ? ampScale : 1.0) * Math.max(0.24, _minMargin / 18.0);
  } else if (rig.bodyType === "blob") {
    ampScale = (typeof ampScale === "number" ? ampScale : 1.0) * 0.55;
  }
  const { pins, bbox, facesLeft, bodyType } = rig;
  if (!rig.hierarchy) rig.hierarchy = buildBoneHierarchy(pins, bodyType);
  const { parentOf, order, sockets } = rig.hierarchy;

  const phase = (frameIdx / Math.max(1, totalFrames)) * Math.PI * 2;
  const dir = facesLeft ? -1 : 1;
  const amp = typeof ampScale === "number" ? ampScale : 1.0;

  // Pair-gap aware non-colliding angle clamp for quadruped/biped legs
  const byId = {};
  for (let i = 0; i < pins.length; i++) byId[pins[i].id] = { pin: pins[i], sock: sockets[i] };
  const clampPairAngle = (id, rawAng) => {
    const partnerId = id === "fl" ? "fr" : (id === "fr" ? "fl" : (id === "bl" ? "br" : (id === "br" ? "bl" : null)));
    if (!partnerId || !byId[id] || !byId[partnerId]) return rawAng;
    const self = byId[id], partner = byId[partnerId];
    const gap = Math.max(4, Math.abs(partner.pin.x - self.pin.x));
    const toward = Math.sign(partner.pin.x - self.pin.x) || 1;
    const tipDx = -Math.sin(rawAng) * self.sock.len;
    if (tipDx * toward > 0) {
      const maxInward = gap * 0.34;
      if (Math.abs(tipDx) > maxInward) return rawAng * (maxInward / Math.abs(tipDx));
    }
    return rawAng;
  };

  // Root translation & local joint angles/offsets per bone
  const localDx = new Float32Array(pins.length);
  const localDy = new Float32Array(pins.length);
  const localAng = new Float32Array(pins.length);

  for (let i = 0; i < pins.length; i++) {
    const id = pins[i].id;
    let dx = 0, dy = 0, ang = 0;

    if (bodyType === "crawler" && id.startsWith("spine_")) {
      const si = parseInt(id.slice(6), 10) || 0;
      const wAmp = (fx === "sprint" ? 1.35 : (fx === "sleep" ? 0.25 : 1.0)) * amp;
      if (si === 0) {
        dy = -Math.abs(Math.sin(phase)) * Math.max(1.0, bbox.bh * 0.04) * wAmp;
        dx = dir * Math.cos(phase) * 0.8 * wAmp;
      } else {
        ang = Math.sin(phase - si * 1.05) * 0.24 * wAmp;
        dy = -Math.abs(Math.sin(phase - si * 1.05)) * Math.max(1.0, bbox.bh * 0.06) * wAmp;
      }
    } else if (bodyType === "swimmer") {
      const wAmp = (fx === "sprint" ? 1.4 : (fx === "sleep" ? 0.3 : 1.0)) * amp;
      const buoy = Math.sin(phase) * Math.max(1.0, bbox.bh * 0.045) * wAmp;
      if (id === "chest") { dy = buoy; ang = Math.sin(phase) * 0.04 * wAmp; }
      else if (id === "head") { ang = -Math.sin(phase) * 0.05 * wAmp; }
      else if (id === "pelvis") { ang = Math.sin(phase - 1.1) * 0.14 * wAmp; }
      else if (id === "tail") { ang = Math.sin(phase - 2.0) * 0.32 * wAmp; }
    } else if (bodyType === "bird") {
      const flap = Math.sin(phase);
      const wAmp = (fx === "sprint" ? 1.35 : (fx === "sleep" ? 0.25 : 1.0)) * amp;
      if (id === "chest") { dy = -flap * Math.max(1.8, bbox.bh * 0.06) * wAmp; ang = dir * flap * 0.05 * wAmp; }
      else if (id === "wing") { ang = -dir * flap * 0.42 * wAmp; dy = flap * Math.max(1.5, bbox.bh * 0.08) * wAmp; }
      else if (id === "head") { ang = dir * Math.sin(phase - 0.6) * 0.08 * wAmp; }
      else if (id === "tail") { ang = dir * Math.sin(phase - 0.9) * 0.18 * wAmp; }
      else if (id === "fl") { ang = -dir * Math.sin(phase) * 0.16 * wAmp; }
    } else if (fx === "walk") {
      const bob = -Math.abs(Math.cos(phase)) * Math.max(1.0, bbox.bh * 0.026) * amp;
      if (id === "chest") { dy = bob; ang = dir * Math.sin(phase) * 0.03 * amp; }
      else if (id === "head") { ang = -dir * Math.sin(phase - 0.5) * 0.07 * amp; dy = -Math.abs(Math.cos(phase - 0.6)) * 0.6 * amp; }
      else if (id === "pelvis") { ang = -dir * Math.sin(phase) * 0.025 * amp; }
      else if (id === "tail") { ang = Math.sin(phase * 2 - 0.6) * 0.18 * amp; }
      else if (id === "l_hand") { ang = -dir * Math.sin(phase + Math.PI) * 0.24 * amp; }
      else if (id === "r_hand") { ang = -dir * Math.sin(phase) * 0.24 * amp; }
      else if (id === "fl" || id === "br") {
        ang = clampPairAngle(id, -dir * Math.sin(phase) * 0.22 * amp);
        dy = -Math.max(0, Math.cos(phase)) * Math.max(1.6, sockets[i].len * 0.16) * amp;
      } else if (id === "fr" || id === "bl") {
        ang = clampPairAngle(id, -dir * Math.sin(phase + Math.PI) * 0.22 * amp);
        dy = -Math.max(0, Math.cos(phase + Math.PI)) * Math.max(1.6, sockets[i].len * 0.16) * amp;
      }
    } else if (fx === "sprint" || fx === "run") {
      const air = -Math.max(0, Math.sin(phase * 2)) * Math.max(2.0, bbox.bh * 0.06) * amp;
      const pitch = dir * Math.cos(phase) * 0.07 * amp;
      if (id === "chest") { dy = air; dx = dir * 0.8 * amp; ang = pitch; }
      else if (id === "head") { ang = -pitch * 0.6; }
      else if (id === "pelvis") { ang = -pitch * 0.5; }
      else if (id === "tail") { ang = Math.sin(phase * 2 - 0.5) * 0.24 * amp; }
      else if (id === "fl") { ang = clampPairAngle(id, -dir * Math.sin(phase) * 0.32 * amp); dy = -Math.max(0, Math.cos(phase)) * Math.max(2.2, sockets[i].len * 0.22) * amp; }
      else if (id === "fr") { ang = clampPairAngle(id, -dir * Math.sin(phase + 0.5) * 0.32 * amp); dy = -Math.max(0, Math.cos(phase + 0.5)) * Math.max(2.2, sockets[i].len * 0.22) * amp; }
      else if (id === "bl") { ang = clampPairAngle(id, -dir * Math.sin(phase + Math.PI * 0.9) * 0.32 * amp); dy = -Math.max(0, Math.cos(phase + Math.PI * 0.9)) * Math.max(2.2, sockets[i].len * 0.22) * amp; }
      else if (id === "br") { ang = clampPairAngle(id, -dir * Math.sin(phase + Math.PI * 1.25) * 0.32 * amp); dy = -Math.max(0, Math.cos(phase + Math.PI * 1.25)) * Math.max(2.2, sockets[i].len * 0.22) * amp; }
      else if (id === "l_hand") { ang = -dir * Math.sin(phase + Math.PI) * 0.32 * amp; }
      else if (id === "r_hand") { ang = -dir * Math.sin(phase) * 0.32 * amp; }
    } else if (fx === "dance") {
      const sway = Math.sin(phase) * Math.max(1.5, bbox.bw * 0.032) * amp;
      const bounce = -Math.abs(Math.sin(phase * 2)) * Math.max(1.6, bbox.bh * 0.048) * amp;
      if (id === "chest") { dx = sway; dy = bounce; ang = Math.sin(phase) * 0.08 * amp; }
      else if (id === "head") { ang = Math.sin(phase - 0.5) * 0.12 * amp; }
      else if (id === "tail") { ang = Math.sin(phase * 2) * 0.24 * amp; }
      else if (id === "fl" || id === "br") { dy = -Math.max(0, Math.sin(phase)) * Math.max(1.5, sockets[i].len * 0.16) * amp; ang = -Math.sin(phase) * 0.06 * amp; }
      else if (id === "fr" || id === "bl") { dy = -Math.max(0, -Math.sin(phase)) * Math.max(1.5, sockets[i].len * 0.16) * amp; ang = Math.sin(phase) * 0.06 * amp; }
      else if (id === "l_hand" || id === "r_hand") { ang = Math.sin(phase * 2) * 0.28 * amp; }
    } else if (fx === "hop") {
      const u = frameIdx / Math.max(1, totalFrames);
      const crouch = u < 0.24 ? Math.sin((u / 0.24) * Math.PI) : (u >= 0.68 ? Math.sin(((u - 0.68) / 0.32) * Math.PI) * 0.7 : 0);
      const air = (u >= 0.24 && u < 0.68) ? Math.sin(((u - 0.24) / 0.44) * Math.PI) : 0;
      if (id === "chest") {
        dy = (crouch * Math.max(2.2, bbox.bh * 0.05) - air * Math.max(4.5, bbox.bh * 0.15)) * amp;
        ang = dir * (crouch * 0.05 - air * 0.06) * amp;
      } else if (id === "fl" || id === "fr") {
        ang = -dir * air * 0.24 * amp;
      } else if (id === "bl" || id === "br") {
        ang = dir * air * 0.24 * amp;
      } else if (id === "tail") {
        ang = -dir * air * 0.22 * amp;
      }
    } else if (fx === "sleep") {
      const br = Math.sin(phase);
      const crouch = Math.max(3.0, bbox.bh * 0.085) * amp;
      if (id === "chest") { dy = crouch - br * Math.max(1.2, bbox.bh * 0.032) * amp; ang = dir * 0.05 * amp; }
      else if (id === "head") { ang = dir * (0.14 + br * 0.04) * amp; dy = crouch * 0.2; }
      else if (id === "tail") { ang = br * 0.10 * amp; }
      else if (id === "fl" || id === "fr") { ang = dir * 0.16 * amp; }
      else if (id === "bl" || id === "br") { ang = -dir * 0.16 * amp; }
    } else if (fx === "pet") {
      const sq = Math.max(0, Math.sin(phase)) * amp;
      if (id === "chest") { dy = sq * Math.max(1.4, bbox.bh * 0.04); }
      else if (id === "head") { dy = sq * Math.max(1.6, bbox.bh * 0.05); ang = dir * sq * 0.10; }
      else if (id === "tail") { ang = Math.sin(phase * 4) * 0.28 * amp; }
    } else if (fx === "breathe") {
      const br = (Math.sin(phase) * 0.5 + 0.5) * amp;
      if (id === "chest") { dy = br * Math.max(1.0, bbox.bh * 0.032); }
      else if (id === "head") { ang = dir * br * 0.04; }
    } else if (fx === "wave") {
      if (id === "tail") { ang = Math.sin(phase) * 0.26 * amp; }
      else if (id === "head") { ang = Math.sin(phase + 1.0) * 0.10 * amp; }
      else if (id === "l_hand") { ang = Math.sin(phase * 2) * 0.35 * amp; }
    }

    localDx[i] = dx;
    localDy[i] = dy;
    localAng[i] = ang;
  }

  // Forward Kinematics: accumulate parent -> child world-space rigid transforms
  // Transform of point (x, y) on bone i:
  //   x' = pivOutX + (x - pivInX)*cos(wAng) - (y - pivInY)*sin(wAng)
  //   y' = pivOutY + (x - pivInX)*sin(wAng) + (y - pivInY)*cos(wAng)
  const world = new Array(pins.length);
  for (let oi = 0; oi < order.length; oi++) {
    const i = order[oi];
    const pi = parentOf[i];
    const sock = sockets[i];
    if (pi === -1) {
      const a = localAng[i];
      world[i] = {
        pivInX: sock.x, pivInY: sock.y,
        pivOutX: sock.x + localDx[i], pivOutY: sock.y + localDy[i],
        ang: a, ca: Math.cos(a), sa: Math.sin(a),
      };
    } else {
      const pw = world[pi];
      const rx = sock.x - pw.pivInX, ry = sock.y - pw.pivInY;
      const sxOut = pw.pivOutX + rx * pw.ca - ry * pw.sa;
      const syOut = pw.pivOutY + rx * pw.sa + ry * pw.ca;
      const a = pw.ang + localAng[i];
      world[i] = {
        pivInX: sock.x, pivInY: sock.y,
        pivOutX: sxOut + localDx[i], pivOutY: syOut + localDy[i],
        ang: a, ca: Math.cos(a), sa: Math.sin(a),
      };
    }
  }
  return world;
}

function renderBoneFkFrame(rig, fx, frameIdx, totalFrames, ampScale) {
  const { grid, epx, w, h, bbox, pins, geo } = rig;
  if (!rig.hierarchy) rig.hierarchy = buildBoneHierarchy(pins, rig.bodyType);
  const { parentOf, depthOf, sockets } = rig.hierarchy;
  const world = computeBoneFkTransforms(rig, fx, frameIdx, totalFrames, ampScale);

  const out = new Uint8ClampedArray(w * h * 4);
  const zBuf = new Int8Array(w * h).fill(-10);
  const ew = epx.w, eh = epx.h, edata = epx.data, efactor = epx.factor || 2;

  const step = Math.max(w, h) > 96 ? 2 : 1;
  const y0 = Math.max(0, bbox.minY), y1 = Math.min(h - 1, bbox.maxY);
  const x0 = Math.max(0, bbox.minX), x1 = Math.min(w - 1, bbox.maxX);

  const evalBonePt = (bi, x, y) => {
    const t = world[bi];
    const rx = x - t.pivInX, ry = y - t.pivInY;
    return {
      x: t.pivOutX + rx * t.ca - ry * t.sa,
      y: t.pivOutY + rx * t.sa + ry * t.ca,
    };
  };

  // Sort bones back-to-front by depthOf
  const drawOrder = Array.from({ length: pins.length }, (_, i) => i).sort((a, b) => depthOf[a] - depthOf[b]);
  const vx = new Float32Array((w + 1) * (h + 1));
  const vy = new Float32Array((w + 1) * (h + 1));
  const blendR = 5.0; // 5px smooth socket blend between parent and child

  for (let di = 0; di < drawOrder.length; di++) {
    const bi = drawOrder[di];
    const pi = parentOf[bi];
    const depth = depthOf[bi];
    const sock = sockets[bi];

    // Find tight bounding box of cells belonging to bone bi (with 1-step overlap collar)
    let bMinX = w, bMaxX = -1, bMinY = h, bMaxY = -1;
    for (let y = y0; y <= y1; y += step) {
      const yNext = Math.min(h, y + step);
      for (let x = x0; x <= x1; x += step) {
        const xNext = Math.min(w, x + step);
        let hit = false;
        for (let ty = y; ty < yNext && !hit; ty++) {
          for (let tx = x; tx < xNext; tx++) {
            const ci = ty * w + tx;
            if (grid[ci * 4 + 3] > 0 && geo.domPin[ci] === bi) { hit = true; break; }
          }
        }
        if (hit) {
          if (x < bMinX) bMinX = x; if (x > bMaxX) bMaxX = x;
          if (y < bMinY) bMinY = y; if (y > bMaxY) bMaxY = y;
        }
      }
    }
    if (bMaxX < bMinX) continue;

    // Expand by 1 step collar for watertight seams
    const sx0 = Math.max(x0, bMinX - step);
    const sx1 = Math.min(x1, bMaxX + step);
    const sy0 = Math.max(y0, bMinY - step);
    const sy1 = Math.min(y1, bMaxY + step);

    // Evaluate vertices on bone bi (smoothly blending to parent near the joint socket)
    for (let y = sy0; y <= sy1 + step; y += step) {
      const cy = Math.min(h, y);
      for (let x = sx0; x <= sx1 + step; x += step) {
        const cx = Math.min(w, x);
        const vi = cy * (w + 1) + cx;
        const pC = evalBonePt(bi, cx, cy);
        if (pi === -1) {
          vx[vi] = pC.x; vy[vi] = pC.y;
        } else {
          // Signed projection along parent->child bone axis
          const dSock = Math.hypot(cx - sock.x, cy - sock.y);
          if (dSock < blendR) {
            const pP = evalBonePt(pi, cx, cy);
            const t = dSock / blendR;
            const s = t * t * (3 - 2 * t);
            vx[vi] = pP.x * (1 - s) + pC.x * s;
            vy[vi] = pP.y * (1 - s) + pC.y * s;
          } else {
            vx[vi] = pC.x; vy[vi] = pC.y;
          }
        }
      }
    }

    // Rasterize bone bi + 1-step overlap collar
    for (let y = sy0; y <= sy1; y += step) {
      const yNext = Math.min(h, y + step);
      for (let x = sx0; x <= sx1; x += step) {
        const xNext = Math.min(w, x + step);
        let belongs = false;
        for (let ty = y; ty < yNext && !belongs; ty++) {
          for (let tx = x; tx < xNext; tx++) {
            const ci = ty * w + tx;
            if (grid[ci * 4 + 3] === 0) continue;
            if (geo.domPin[ci] === bi) { belongs = true; break; }
            const lx = Math.max(0, tx - step), rx = Math.min(w - 1, tx + step);
            const uy = Math.max(0, ty - step), dy = Math.min(h - 1, ty + step);
            if (
              geo.domPin[ty * w + lx] === bi || geo.domPin[ty * w + rx] === bi ||
              geo.domPin[uy * w + tx] === bi || geo.domPin[dy * w + tx] === bi
            ) { belongs = true; break; }
          }
        }
        if (!belongs) continue;
        const i00 = y * (w + 1) + x, i10 = y * (w + 1) + xNext;
        const i01 = yNext * (w + 1) + x, i11 = yNext * (w + 1) + xNext;
        rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vx[i00], vy[i00], x, y, vx[i10], vy[i10], xNext, y, vx[i11], vy[i11], xNext, yNext, depth);
        rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vx[i00], vy[i00], x, y, vx[i11], vy[i11], xNext, yNext, vx[i01], vy[i01], x, yNext, depth);
      }
    }
  }

  // Strict ground-baseline alignment for grounded cycles
  let finalOut = out;
  if ((fx === "walk" || fx === "breathe" || fx === "wave" || fx === "sleep") && rig.bodyType !== "bird" && rig.bodyType !== "swimmer") {
    let curFoot = -1;
    for (let y = h - 1; y >= 0 && curFoot === -1; y--) {
      for (let x = 0; x < w; x++) {
        if (out[(y * w + x) * 4 + 3] > 0) { curFoot = y; break; }
      }
    }
    if (curFoot !== -1 && curFoot !== bbox.maxY) {
      const shift = bbox.maxY - curFoot;
      if (Math.abs(shift) <= 14) {
        const aligned = new Uint8ClampedArray(w * h * 4);
        for (let y = 0; y < h; y++) {
          const ty = y + shift;
          if (ty < 0 || ty >= h) continue;
          for (let x = 0; x < w; x++) {
            const sp = (y * w + x) * 4, dp = (ty * w + x) * 4;
            aligned[dp] = out[sp]; aligned[dp + 1] = out[sp + 1]; aligned[dp + 2] = out[sp + 2]; aligned[dp + 3] = out[sp + 3];
          }
        }
        finalOut = aligned;
      }
    }
  }

  sealPinholesAndSpecks(finalOut, w, h);
  const lockEl = typeof document !== "undefined" ? document.getElementById("gif-outline-lock") : null;
  if ((!lockEl || lockEl.checked) && !rig.isHiResNative) {
    applyCrispOutlineLock(finalOut, w, h, rig.inkColor || [20, 22, 28]);
  }
  return finalOut;
}


// ============================================================================
// RESEARCH-BACKED 2D RIGGING SUITE V3 (TRUE PAPER IMPLEMENTATIONS)
// 1. harmonic_dqs      : Bounded Biharmonic Weights (Jacobson et al. SIGGRAPH 2011)
//                        via bone-segment Dirichlet seeds + 13-pt biharmonic relaxation
// 2. fabrik_ik         : Iterative FABRIK 2D Solver (Aristidou & Lasenby 2011)
//                        forward + backward reaching with knee/ankle joint constraints
// 3. layered_inpaint   : Limb-Subtracted Torso Inpainting (Meta Animated Drawings 2023)
// 4. complementary_dyn : Per-Bone 2nd-Order Spring-Damper Lag (Zhang et al. SIGGRAPH 2020)
// ============================================================================

function alignAndSealOutput(out, rig, fx) {
  const { w, h, bbox } = rig;
  let finalOut = out;
  if ((fx === "walk" || fx === "breathe" || fx === "wave" || fx === "sleep") && rig.bodyType !== "bird" && rig.bodyType !== "swimmer") {
    let curFoot = -1;
    for (let y = h - 1; y >= 0 && curFoot === -1; y--) {
      for (let x = 0; x < w; x++) {
        if (out[(y * w + x) * 4 + 3] > 0) { curFoot = y; break; }
      }
    }
    if (curFoot !== -1 && curFoot !== bbox.maxY) {
      const shift = bbox.maxY - curFoot;
      if (Math.abs(shift) <= 14) {
        const aligned = new Uint8ClampedArray(w * h * 4);
        for (let y = 0; y < h; y++) {
          const ty = y + shift;
          if (ty < 0 || ty >= h) continue;
          for (let x = 0; x < w; x++) {
            const sp = (y * w + x) * 4, dp = (ty * w + x) * 4;
            aligned[dp] = out[sp]; aligned[dp + 1] = out[sp + 1]; aligned[dp + 2] = out[sp + 2]; aligned[dp + 3] = out[sp + 3];
          }
        }
        finalOut = aligned;
      }
    }
  }
  const lockEl = typeof document !== "undefined" ? document.getElementById("gif-outline-lock") : null;
  if ((!lockEl || lockEl.checked) && !rig.isHiResNative) {
    applyCrispOutlineLock(finalOut, w, h, rig.inkColor || [20, 22, 28]);
  }
  // Always run pinhole & speck seal LAST so no post-pass can re-open a 1px hole
  sealPinholesAndSpecks(finalOut, w, h);
  return finalOut;
}

// --- METHOD 1 V3: BOUNDED BIHARMONIC WEIGHTS (BONE-SEGMENT SEEDS + BIHARMONIC STENCIL) ---
function computeBiharmonicWeights(rig) {
  if (rig.bbwWeights) return rig.bbwWeights;
  const { grid, w, h, pins, geo } = rig;
  if (!rig.hierarchy) rig.hierarchy = buildBoneHierarchy(pins, rig.bodyType);
  const { sockets } = rig.hierarchy;
  const numPins = pins.length;
  const n = w * h;
  const weights = new Array(numPins);
  const isSeed = new Uint8Array(n);
  const seedPin = new Int16Array(n).fill(-1);

  // Seed along each FULL bone segment (socket -> pin tip), not just a point dot
  for (let i = 0; i < numPins; i++) {
    weights[i] = new Float32Array(n);
    const sx = sockets[i] ? sockets[i].x : pins[i].x;
    const sy = sockets[i] ? sockets[i].y : pins[i].y;
    const ex = pins[i].x, ey = pins[i].y;
    const steps = Math.max(1, Math.ceil(Math.hypot(ex - sx, ey - sy)));
    for (let s = 0; s <= steps; s++) {
      const t = s / steps;
      const bx = Math.round(sx + (ex - sx) * t);
      const by = Math.round(sy + (ey - sy) * t);
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          const nx = bx + dx, ny = by + dy;
          if (nx >= 0 && nx < w && ny >= 0 && ny < h) {
            const idx = ny * w + nx;
            if (grid[idx * 4 + 3] > 0) {
              isSeed[idx] = 1;
              seedPin[idx] = i;
            }
          }
        }
      }
    }
  }

  for (let idx = 0; idx < n; idx++) {
    if (grid[idx * 4 + 3] === 0) continue;
    const dp = isSeed[idx] ? seedPin[idx] : geo.domPin[idx];
    if (dp >= 0 && dp < numPins) weights[dp][idx] = 1.0;
  }

  // Stage 1: Harmonic warm-start (18 SOR passes)
  const omega = 1.6;
  for (let iter = 0; iter < 18; iter++) {
    for (let y = 1; y < h - 1; y++) {
      for (let x = 1; x < w - 1; x++) {
        const idx = y * w + x;
        if (grid[idx * 4 + 3] === 0 || isSeed[idx]) continue;
        const nL = grid[(idx - 1) * 4 + 3] > 0 ? idx - 1 : -1;
        const nR = grid[(idx + 1) * 4 + 3] > 0 ? idx + 1 : -1;
        const nU = grid[(idx - w) * 4 + 3] > 0 ? idx - w : -1;
        const nD = grid[(idx + w) * 4 + 3] > 0 ? idx + w : -1;
        const cnt = (nL >= 0 ? 1 : 0) + (nR >= 0 ? 1 : 0) + (nU >= 0 ? 1 : 0) + (nD >= 0 ? 1 : 0);
        if (cnt === 0) continue;
        const invCnt = 1.0 / cnt;
        for (let p = 0; p < numPins; p++) {
          const wp = weights[p];
          let sum = 0;
          if (nL >= 0) sum += wp[nL];
          if (nR >= 0) sum += wp[nR];
          if (nU >= 0) sum += wp[nU];
          if (nD >= 0) sum += wp[nD];
          const next = wp[idx] + omega * (sum * invCnt - wp[idx]);
          wp[idx] = next < 0 ? 0 : (next > 1 ? 1 : next);
        }
      }
    }
  }

  // Stage 2: Biharmonic 13-point stencil smoothing (Delta^2 w = 0, clamped to [0, 1])
  // Gives C1-continuous, non-negative, partition-of-unity Bounded Biharmonic Weights
  for (let iter = 0; iter < 14; iter++) {
    for (let y = 2; y < h - 2; y++) {
      for (let x = 2; x < w - 2; x++) {
        const idx = y * w + x;
        if (grid[idx * 4 + 3] === 0 || isSeed[idx]) continue;
        if (
          grid[(idx - 1) * 4 + 3] === 0 || grid[(idx + 1) * 4 + 3] === 0 ||
          grid[(idx - w) * 4 + 3] === 0 || grid[(idx + w) * 4 + 3] === 0
        ) continue;
        const hasRing2 =
          grid[(idx - 2) * 4 + 3] > 0 && grid[(idx + 2) * 4 + 3] > 0 &&
          grid[(idx - 2 * w) * 4 + 3] > 0 && grid[(idx + 2 * w) * 4 + 3] > 0 &&
          grid[(idx - w - 1) * 4 + 3] > 0 && grid[(idx - w + 1) * 4 + 3] > 0 &&
          grid[(idx + w - 1) * 4 + 3] > 0 && grid[(idx + w + 1) * 4 + 3] > 0;
        if (!hasRing2) continue;

        for (let p = 0; p < numPins; p++) {
          const wp = weights[p];
          const ring1 = wp[idx - 1] + wp[idx + 1] + wp[idx - w] + wp[idx + w];
          const diag = wp[idx - w - 1] + wp[idx - w + 1] + wp[idx + w - 1] + wp[idx + w + 1];
          const ring2 = wp[idx - 2] + wp[idx + 2] + wp[idx - 2 * w] + wp[idx + 2 * w];
          const biharm = (8 * ring1 - 2 * diag - ring2) / 20.0;
          const next = wp[idx] + 0.55 * (biharm - wp[idx]);
          wp[idx] = next < 0 ? 0 : (next > 1 ? 1 : next);
        }
      }
    }
  }

  // Power-sharpen BBW weights (gamma = 2.6) so legs retain full stride authority while sockets stay smooth
  for (let idx = 0; idx < n; idx++) {
    if (grid[idx * 4 + 3] === 0) continue;
    let total = 0;
    for (let p = 0; p < numPins; p++) {
      const v = weights[p][idx];
      const sv = v > 1e-5 ? Math.pow(v, 2.6) : 0;
      weights[p][idx] = sv;
      total += sv;
    }
    if (total > 1e-6) {
      const invT = 1.0 / total;
      for (let p = 0; p < numPins; p++) weights[p][idx] *= invT;
    } else {
      const dp = Math.max(0, Math.min(numPins - 1, geo.domPin[idx]));
      weights[dp][idx] = 1.0;
    }
  }
  rig.bbwWeights = weights;
  return weights;
}

function evalBbwRigidMesh(rig, world, weights, vx, vy, step, squashX, squashY) {
  const { w, h, bbox, pins } = rig;
  const y0 = Math.max(0, bbox.minY), y1 = Math.min(h - 1, bbox.maxY);
  const x0 = Math.max(0, bbox.minX), x1 = Math.min(w - 1, bbox.maxX);
  const cxMid = (bbox.minX + bbox.maxX) * 0.5;
  const cyFoot = bbox.maxY;

  for (let y = y0; y <= y1 + step; y += step) {
    const cy = Math.min(h, y);
    const sy = Math.min(h - 1, cy);
    for (let x = x0; x <= x1 + step; x += step) {
      const cx = Math.min(w, x);
      const sx = Math.min(w - 1, cx);
      let idx = sy * w + sx;
      if (rig.grid[idx * 4 + 3] === 0) {
        if (sx > 0 && rig.grid[(sy * w + (sx - 1)) * 4 + 3] > 0) idx = sy * w + (sx - 1);
        else if (sy > 0 && rig.grid[((sy - 1) * w + sx) * 4 + 3] > 0) idx = (sy - 1) * w + sx;
        else if (sx > 0 && sy > 0 && rig.grid[((sy - 1) * w + (sx - 1)) * 4 + 3] > 0) idx = (sy - 1) * w + (sx - 1);
      }

      let sumX = 0, sumY = 0, sumW = 0;
      for (let p = 0; p < pins.length; p++) {
        const wp = weights[p][idx];
        if (wp < 1e-4) continue;
        const t = world[p];
        const rx = cx - t.pivInX, ry = cy - t.pivInY;
        sumX += wp * (t.pivOutX + rx * t.ca - ry * t.sa);
        sumY += wp * (t.pivOutY + rx * t.sa + ry * t.ca);
        sumW += wp;
      }

      const vi = cy * (w + 1) + cx;
      let outX = cx, outY = cy;
      if (sumW > 1e-4) {
        outX = sumX / sumW;
        outY = sumY / sumW;
      }
      if (squashX !== 1.0 || squashY !== 1.0) {
        outX = cxMid + (outX - cxMid) * squashX;
        outY = cyFoot + (outY - cyFoot) * squashY;
      }
      vx[vi] = outX;
      vy[vi] = outY;
    }
  }
}

function renderHarmonicDqsFrame(rig, fx, frameIdx, totalFrames, ampScale) {
  const { grid, epx, w, h, bbox } = rig;
  const world = computeBoneFkTransforms(rig, fx, frameIdx, totalFrames, ampScale);
  const weights = computeBiharmonicWeights(rig);

  const out = new Uint8ClampedArray(w * h * 4);
  const zBuf = new Int8Array(w * h).fill(-10);
  const ew = epx.w, eh = epx.h, edata = epx.data, efactor = epx.factor || 2;
  const step = Math.max(w, h) > 96 ? 2 : 1;
  const y0 = Math.max(0, bbox.minY), y1 = Math.min(h - 1, bbox.maxY);
  const x0 = Math.max(0, bbox.minX), x1 = Math.min(w - 1, bbox.maxX);

  const vx = new Float32Array((w + 1) * (h + 1));
  const vy = new Float32Array((w + 1) * (h + 1));
  evalBbwRigidMesh(rig, world, weights, vx, vy, step, 1.0, 1.0);

  for (let y = y0; y <= y1; y += step) {
    const yNext = Math.min(h, y + step);
    for (let x = x0; x <= x1; x += step) {
      const xNext = Math.min(w, x + step);
      let hasOpaque = false;
      for (let ty = y; ty < yNext && !hasOpaque; ty++) {
        for (let tx = x; tx < xNext; tx++) {
          if (grid[(ty * w + tx) * 4 + 3] > 0) { hasOpaque = true; break; }
        }
      }
      if (!hasOpaque) continue;
      const i00 = y * (w + 1) + x, i10 = y * (w + 1) + xNext;
      const i01 = yNext * (w + 1) + x, i11 = yNext * (w + 1) + xNext;
      rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vx[i00], vy[i00], x, y, vx[i10], vy[i10], xNext, y, vx[i11], vy[i11], xNext, yNext, 3);
      rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vx[i00], vy[i00], x, y, vx[i11], vy[i11], xNext, yNext, vx[i01], vy[i01], x, yNext, 3);
    }
  }
  return alignAndSealOutput(out, rig, fx);
}

// --- METHOD 2 V3: TRUE ITERATIVE FABRIK 2D SOLVER (ARISTIDOU & LASENBY 2011) ---
function solveFabrikChain2D(joints, lengths, targetX, targetY, bendDir, maxIters) {
  const n = joints.length;
  const rootX = joints[0].x, rootY = joints[0].y;
  const pts = joints.map((p) => ({ x: p.x, y: p.y }));
  let totalLen = 0;
  for (let i = 0; i < lengths.length; i++) totalLen += lengths[i];

  const distToTarget = Math.hypot(targetX - rootX, targetY - rootY);
  if (distToTarget >= totalLen - 1e-3) {
    // Target unreachable: stretch chain straight toward target
    const ux = (targetX - rootX) / Math.max(1e-5, distToTarget);
    const uy = (targetY - rootY) / Math.max(1e-5, distToTarget);
    for (let i = 0; i < n - 1; i++) {
      pts[i + 1].x = pts[i].x + ux * lengths[i];
      pts[i + 1].y = pts[i].y + uy * lengths[i];
    }
    return pts;
  }

  // Bias initial mid-joint in anatomical knee bend direction (safe when target == root)
  if (n === 3) {
    const mx = (rootX + targetX) * 0.5, my = (rootY + targetY) * 0.5;
    const dx = targetX - rootX, dy = targetY - rootY;
    const d = Math.hypot(dx, dy);
    if (d < 1e-3) {
      pts[1].x = rootX + (bendDir || 1) * lengths[0];
      pts[1].y = rootY;
    } else {
      pts[1].x = mx + (-dy / d) * bendDir * 4.0;
      pts[1].y = my + (dx / d) * bendDir * 4.0;
    }
  }

  for (let iter = 0; iter < maxIters; iter++) {
    // Forward reaching: set end-effector to target, pull parents
    pts[n - 1].x = targetX;
    pts[n - 1].y = targetY;
    for (let i = n - 2; i >= 0; i--) {
      const r = Math.max(1e-5, Math.hypot(pts[i].x - pts[i + 1].x, pts[i].y - pts[i + 1].y));
      const lam = lengths[i] / r;
      pts[i].x = (1 - lam) * pts[i + 1].x + lam * pts[i].x;
      pts[i].y = (1 - lam) * pts[i + 1].y + lam * pts[i].y;
    }
    // Backward reaching: pin root back to socket, push children
    pts[0].x = rootX;
    pts[0].y = rootY;
    for (let i = 0; i < n - 1; i++) {
      const r = Math.max(1e-5, Math.hypot(pts[i + 1].x - pts[i].x, pts[i + 1].y - pts[i].y));
      const lam = lengths[i] / r;
      pts[i + 1].x = (1 - lam) * pts[i].x + lam * pts[i + 1].x;
      pts[i + 1].y = (1 - lam) * pts[i].y + lam * pts[i + 1].y;
    }
    if (Math.hypot(pts[n - 1].x - targetX, pts[n - 1].y - targetY) < 0.08) break;
  }
  return pts;
}

function renderFabrikIkFrame(rig, fx, frameIdx, totalFrames, ampScale) {
  const { grid, epx, w, h, bbox, pins, geo, facesLeft } = rig;
  if (!rig.hierarchy) rig.hierarchy = buildBoneHierarchy(pins, rig.bodyType);
  const { parentOf, depthOf, sockets } = rig.hierarchy;
  const world = computeBoneFkTransforms(rig, fx, frameIdx, totalFrames, ampScale);

  const uBase = (frameIdx % Math.max(1, totalFrames)) / Math.max(1, totalFrames);
  const dir = facesLeft ? -1 : 1;
  const amp = typeof ampScale === "number" ? ampScale : 1.0;

  const ikMap = new Map();
  const useIk = (fx === "walk" || fx === "sprint" || fx === "run" || fx === "hop");
  if (useIk) {
    for (let i = 0; i < pins.length; i++) {
      const id = pins[i].id;
      if (id !== "fl" && id !== "fr" && id !== "bl" && id !== "br") continue;
      const sock = sockets[i];
      const pi = parentOf[i];
      const pw = pi >= 0 ? world[pi] : world[i];
      const rx = sock.x - pw.pivInX, ry = sock.y - pw.pivInY;
      const hxOut = pw.pivOutX + rx * pw.ca - ry * pw.sa;
      const hyOut = pw.pivOutY + rx * pw.sa + ry * pw.ca;

      const pawRestX = pins[i].x, pawRestY = Math.min(bbox.maxY, pins[i].y + sock.len * 0.55);
      const legLen = Math.max(8, Math.hypot(pawRestX - sock.x, pawRestY - sock.y));
      const l1 = legLen * 0.52, l2 = legLen * 0.50;

      const u = (uBase + ((id === "fl" || id === "br") ? 0.0 : 0.5)) % 1.0;
      const stride = Math.min(7.5, (fx === "sprint" || fx === "run" ? 0.24 : 0.18) * legLen * amp);
      const lift = Math.min(5.0, (fx === "sprint" || fx === "run" ? 0.18 : 0.12) * legLen * amp);

      let swingX = 0, liftY = 0;
      if (u < 0.6) {
        const s = u / 0.6;
        swingX = (1 - 2 * s) * stride;
        liftY = 0;
      } else {
        const s = (u - 0.6) / 0.4;
        swingX = (-1 + 2 * s) * stride;
        liftY = Math.sin(s * Math.PI) * lift;
      }

      const tx = pawRestX + dir * swingX;
      const ty = pawRestY - liftY;
      const bendDir = (id === "fl" || id === "fr") ? -dir : dir;

      const restChain = solveFabrikChain2D(
        [{ x: sock.x, y: sock.y }, { x: (sock.x + pawRestX) * 0.5, y: (sock.y + pawRestY) * 0.5 }, { x: pawRestX, y: pawRestY }],
        [l1, l2], pawRestX, pawRestY, bendDir, 6
      );
      const poseChain = solveFabrikChain2D(
        [{ x: hxOut, y: hyOut }, { x: (hxOut + tx) * 0.5, y: (hyOut + ty) * 0.5 }, { x: tx, y: ty }],
        [l1, l2], tx, ty, bendDir, 6
      );

      const restThigh = Math.atan2(restChain[1].y - restChain[0].y, restChain[1].x - restChain[0].x);
      const restShin = Math.atan2(restChain[2].y - restChain[1].y, restChain[2].x - restChain[1].x);
      const poseThigh = Math.atan2(poseChain[1].y - poseChain[0].y, poseChain[1].x - poseChain[0].x);
      const poseShin = Math.atan2(poseChain[2].y - poseChain[1].y, poseChain[2].x - poseChain[1].x);

      let dThigh = poseThigh - restThigh;
      let dShin = poseShin - restShin;
      // Wrap angle deltas to [-pi, pi] and clamp knee flexion to prevent stubby-leg spikes
      while (dThigh > Math.PI) dThigh -= Math.PI * 2;
      while (dThigh < -Math.PI) dThigh += Math.PI * 2;
      while (dShin > Math.PI) dShin -= Math.PI * 2;
      while (dShin < -Math.PI) dShin += Math.PI * 2;
      dThigh = Math.max(-0.38, Math.min(0.38, dThigh));
      dShin = Math.max(dThigh - 0.35, Math.min(dThigh + 0.35, dShin));

      // Recompute welded knee output from clamped dThigh so thigh & shin can never separate
      const kRx = restChain[1].x - restChain[0].x;
      const kRy = restChain[1].y - restChain[0].y;
      const weldedKxOut = poseChain[0].x + kRx * Math.cos(dThigh) - kRy * Math.sin(dThigh);
      const weldedKyOut = poseChain[0].y + kRx * Math.sin(dThigh) + kRy * Math.cos(dThigh);

      ikMap.set(i, {
        hxIn: restChain[0].x, hyIn: restChain[0].y, kxIn: restChain[1].x, kyIn: restChain[1].y,
        hxOut: poseChain[0].x, hyOut: poseChain[0].y, kxOut: weldedKxOut, kyOut: weldedKyOut,
        dThigh,
        dShin,
        l1,
      });
    }
  }

  const out = new Uint8ClampedArray(w * h * 4);
  const zBuf = new Int8Array(w * h).fill(-10);
  const ew = epx.w, eh = epx.h, edata = epx.data, efactor = epx.factor || 2;
  const step = Math.max(w, h) > 96 ? 2 : 1;
  const y0 = Math.max(0, bbox.minY), y1 = Math.min(h - 1, bbox.maxY);
  const x0 = Math.max(0, bbox.minX), x1 = Math.min(w - 1, bbox.maxX);

  const evalPt = (bi, cx, cy) => {
    const ik = ikMap.get(bi);
    if (ik) {
      const dHip = Math.hypot(cx - ik.hxIn, cy - ik.hyIn);
      const caT = Math.cos(ik.dThigh), saT = Math.sin(ik.dThigh);
      const rxT = cx - ik.hxIn, ryT = cy - ik.hyIn;
      const pThighX = ik.hxOut + rxT * caT - ryT * saT;
      const pThighY = ik.hyOut + rxT * saT + ryT * caT;

      const caS = Math.cos(ik.dShin), saS = Math.sin(ik.dShin);
      const rxS = cx - ik.kxIn, ryS = cy - ik.kyIn;
      const pShinX = ik.kxOut + rxS * caS - ryS * saS;
      const pShinY = ik.kyOut + rxS * saS + ryS * caS;

      const kneeBand = 4.5;
      const t = Math.max(0, Math.min(1, (dHip - (ik.l1 - kneeBand * 0.5)) / kneeBand));
      const s = t * t * (3 - 2 * t);
      return { x: pThighX * (1 - s) + pShinX * s, y: pThighY * (1 - s) + pShinY * s };
    }
    const t = world[bi];
    const rx = cx - t.pivInX, ry = cy - t.pivInY;
    return { x: t.pivOutX + rx * t.ca - ry * t.sa, y: t.pivOutY + rx * t.sa + ry * t.ca };
  };

  const drawOrder = Array.from({ length: pins.length }, (_, i) => i).sort((a, b) => depthOf[a] - depthOf[b]);
  const vx = new Float32Array((w + 1) * (h + 1));
  const vy = new Float32Array((w + 1) * (h + 1));
  const blendR = 5.5;

  for (let di = 0; di < drawOrder.length; di++) {
    const bi = drawOrder[di];
    const pi = parentOf[bi];
    const depth = depthOf[bi];
    const sock = sockets[bi];

    let bMinX = w, bMaxX = -1, bMinY = h, bMaxY = -1;
    for (let y = y0; y <= y1; y += step) {
      const yNext = Math.min(h, y + step);
      for (let x = x0; x <= x1; x += step) {
        const xNext = Math.min(w, x + step);
        let hit = false;
        for (let ty = y; ty < yNext && !hit; ty++) {
          for (let tx = x; tx < xNext; tx++) {
            const ci = ty * w + tx;
            if (grid[ci * 4 + 3] > 0 && geo.domPin[ci] === bi) { hit = true; break; }
          }
        }
        if (hit) {
          if (x < bMinX) bMinX = x; if (x > bMaxX) bMaxX = x;
          if (y < bMinY) bMinY = y; if (y > bMaxY) bMaxY = y;
        }
      }
    }
    if (bMaxX < bMinX) continue;
    const sx0 = Math.max(x0, bMinX - step), sx1 = Math.min(x1, bMaxX + step);
    const sy0 = Math.max(y0, bMinY - step), sy1 = Math.min(y1, bMaxY + step);

    for (let y = sy0; y <= sy1 + step; y += step) {
      const cy = Math.min(h, y);
      for (let x = sx0; x <= sx1 + step; x += step) {
        const cx = Math.min(w, x);
        const vi = cy * (w + 1) + cx;
        const pC = evalPt(bi, cx, cy);
        if (pi === -1) {
          vx[vi] = pC.x; vy[vi] = pC.y;
        } else {
          const dSock = Math.hypot(cx - sock.x, cy - sock.y);
          if (dSock < blendR) {
            const pP = evalPt(pi, cx, cy);
            const t = dSock / blendR;
            const s = t * t * (3 - 2 * t);
            vx[vi] = pP.x * (1 - s) + pC.x * s;
            vy[vi] = pP.y * (1 - s) + pC.y * s;
          } else {
            vx[vi] = pC.x; vy[vi] = pC.y;
          }
        }
      }
    }

    for (let y = sy0; y <= sy1; y += step) {
      const yNext = Math.min(h, y + step);
      for (let x = sx0; x <= sx1; x += step) {
        const xNext = Math.min(w, x + step);
        let belongs = false;
        for (let ty = y; ty < yNext && !belongs; ty++) {
          for (let tx = x; tx < xNext; tx++) {
            const ci = ty * w + tx;
            if (grid[ci * 4 + 3] === 0) continue;
            if (geo.domPin[ci] === bi) { belongs = true; break; }
            const lx = Math.max(0, tx - step), rx = Math.min(w - 1, tx + step);
            const uy = Math.max(0, ty - step), dy = Math.min(h - 1, ty + step);
            if (
              geo.domPin[ty * w + lx] === bi || geo.domPin[ty * w + rx] === bi ||
              geo.domPin[uy * w + tx] === bi || geo.domPin[dy * w + tx] === bi
            ) { belongs = true; break; }
          }
        }
        if (!belongs) continue;
        const i00 = y * (w + 1) + x, i10 = y * (w + 1) + xNext;
        const i01 = yNext * (w + 1) + x, i11 = yNext * (w + 1) + xNext;
        rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vx[i00], vy[i00], x, y, vx[i10], vy[i10], xNext, y, vx[i11], vy[i11], xNext, yNext, depth);
        rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vx[i00], vy[i00], x, y, vx[i11], vy[i11], xNext, yNext, vx[i01], vy[i01], x, yNext, depth);
      }
    }
  }
  return alignAndSealOutput(out, rig, fx);
}

// --- METHOD 3 V3: LIMB-SUBTRACTED TORSO INPAINTING (META ANIMATED DRAWINGS STYLE) ---
function inpaintTorsoBacking(rig) {
  if (rig.inpaintedEpx) return rig.inpaintedEpx;
  const { grid, w, h, pins, geo } = rig;
  const buf = new Uint8ClampedArray(w * h * 4);
  const isLimb = new Uint8Array(pins.length);
  for (let i = 0; i < pins.length; i++) {
    const id = pins[i].id;
    if (id === "fl" || id === "fr" || id === "bl" || id === "br" || id === "l_hand" || id === "r_hand" || id === "wing") {
      isLimb[i] = 1;
    }
  }

  // Step 1: Subtract limbs completely from torso layer & compute interior torso fill color
  const mask = new Uint8Array(w * h);
  let sumTR = 0, sumTG = 0, sumTB = 0, cntT = 0;
  let allR = 0, allG = 0, allB = 0, allCnt = 0;
  for (let i = 0; i < w * h; i++) {
    if (grid[i * 4 + 3] > 0 && !isLimb[geo.domPin[i]]) {
      mask[i] = 1;
      const p = i * 4;
      buf[p] = grid[p]; buf[p + 1] = grid[p + 1]; buf[p + 2] = grid[p + 2]; buf[p + 3] = 255;
      allR += grid[p]; allG += grid[p + 1]; allB += grid[p + 2]; allCnt++;
      const lum = grid[p] * 0.299 + grid[p + 1] * 0.587 + grid[p + 2] * 0.114;
      if (lum > 65) {
        sumTR += grid[p]; sumTG += grid[p + 1]; sumTB += grid[p + 2]; cntT++;
      }
    }
  }
  const fillR = cntT > 0 ? (sumTR / cntT) | 0 : (allCnt > 0 ? (allR / allCnt) | 0 : 40);
  const fillG = cntT > 0 ? (sumTG / cntT) | 0 : (allCnt > 0 ? (allG / allCnt) | 0 : 40);
  const fillB = cntT > 0 ? (sumTB / cntT) | 0 : (allCnt > 0 ? (allB / allCnt) | 0 : 40);

  // Step 2: 6-pass boundary-aware interior-color inpainting into the subtracted limb socket
  for (let pass = 0; pass < 6; pass++) {
    const nextMask = new Uint8Array(mask);
    const snap = new Uint8ClampedArray(buf);
    for (let y = 1; y < h - 1; y++) {
      for (let x = 1; x < w - 1; x++) {
        const i = y * w + x;
        if (mask[i] || grid[i * 4 + 3] === 0) continue;
        let r = 0, g = 0, b = 0, cnt = 0;
        let fr = 0, fg = 0, fb = 0, fcnt = 0;
        const nbrs = [i - 1, i + 1, i - w, i + w];
        for (let k = 0; k < 4; k++) {
          const ni = nbrs[k];
          if (mask[ni]) {
            const p = ni * 4;
            const lum = snap[p] * 0.299 + snap[p + 1] * 0.587 + snap[p + 2] * 0.114;
            if (lum > 65) {
              fr += snap[p]; fg += snap[p + 1]; fb += snap[p + 2]; fcnt++;
            }
            r += snap[p]; g += snap[p + 1]; b += snap[p + 2]; cnt++;
          }
        }
        if (cnt > 0) {
          const p = i * 4;
          if (fcnt > 0) {
            buf[p] = (fr / fcnt) | 0; buf[p + 1] = (fg / fcnt) | 0; buf[p + 2] = (fb / fcnt) | 0;
          } else {
            buf[p] = fillR; buf[p + 1] = fillG; buf[p + 2] = fillB;
          }
          buf[p + 3] = 255;
          nextMask[i] = 1;
        }
      }
    }
    mask.set(nextMask);
  }
  let hasLimbs = false;
  for (let i = 0; i < isLimb.length; i++) if (isLimb[i]) { hasLimbs = true; break; }
  rig.inpaintedHasLimbs = hasLimbs;
  rig.inpaintedMask = mask;
  rig.inpaintedEpx = { data: buf, w, h, factor: 1 };
  return rig.inpaintedEpx;
}

function renderLayeredInpaintFrame(rig, fx, frameIdx, totalFrames, ampScale) {
  const wideAmp = (typeof ampScale === "number" ? ampScale : 1.0) * 1.14;
  const { grid, epx, w, h, bbox, pins, geo } = rig;
  if (!rig.hierarchy) rig.hierarchy = buildBoneHierarchy(pins, rig.bodyType);
  const { parentOf, depthOf, sockets } = rig.hierarchy;
  const world = computeBoneFkTransforms(rig, fx, frameIdx, totalFrames, wideAmp);
  const inp = inpaintTorsoBacking(rig);

  const out = new Uint8ClampedArray(w * h * 4);
  const zBuf = new Int8Array(w * h).fill(-10);
  const step = Math.max(w, h) > 96 ? 2 : 1;
  const y0 = Math.max(0, bbox.minY), y1 = Math.min(h - 1, bbox.maxY);
  const x0 = Math.max(0, bbox.minX), x1 = Math.min(w - 1, bbox.maxX);

  const evalBone = (bi, x, y) => {
    const t = world[bi];
    const rx = x - t.pivInX, ry = y - t.pivInY;
    return { x: t.pivOutX + rx * t.ca - ry * t.sa, y: t.pivOutY + rx * t.sa + ry * t.ca };
  };

  const chestIdx = Math.max(0, pins.findIndex((p) => p.id === "chest"));
  const vx = new Float32Array((w + 1) * (h + 1));
  const vy = new Float32Array((w + 1) * (h + 1));
  if (rig.inpaintedHasLimbs) {
  for (let y = y0; y <= y1 + step; y += step) {
    const cy = Math.min(h, y);
    for (let x = x0; x <= x1 + step; x += step) {
      const cx = Math.min(w, x);
      const pt = evalBone(chestIdx, cx, cy);
      vx[cy * (w + 1) + cx] = pt.x;
      vy[cy * (w + 1) + cx] = pt.y;
    }
  }
  for (let y = y0; y <= y1; y += step) {
    const yNext = Math.min(h, y + step);
    for (let x = x0; x <= x1; x += step) {
      const xNext = Math.min(w, x + step);
      let hit = false;
      for (let ty = y; ty < yNext && !hit; ty++) {
        for (let tx = x; tx < xNext; tx++) {
          if (rig.inpaintedMask[ty * w + tx]) { hit = true; break; }
        }
      }
      if (!hit) continue;
      const i00 = y * (w + 1) + x, i10 = y * (w + 1) + xNext;
      const i01 = yNext * (w + 1) + x, i11 = yNext * (w + 1) + xNext;
      rasterizeTriEpx(out, zBuf, w, h, inp.data, inp.w, inp.h, 1, vx[i00], vy[i00], x, y, vx[i10], vy[i10], xNext, y, vx[i11], vy[i11], xNext, yNext, 3);
      rasterizeTriEpx(out, zBuf, w, h, inp.data, inp.w, inp.h, 1, vx[i00], vy[i00], x, y, vx[i11], vy[i11], xNext, yNext, vx[i01], vy[i01], x, yNext, 3);
    }
  }
  }

  const ew = epx.w, eh = epx.h, edata = epx.data, efactor = epx.factor || 2;
  const drawOrder = Array.from({ length: pins.length }, (_, i) => i).sort((a, b) => depthOf[a] - depthOf[b]);
  const blendR = 4.5;
  for (let di = 0; di < drawOrder.length; di++) {
    const bi = drawOrder[di];
    const pi = parentOf[bi];
    const sock = sockets[bi];
    const depth = depthOf[bi];
    for (let y = y0; y <= y1 + step; y += step) {
      const cy = Math.min(h, y);
      for (let x = x0; x <= x1 + step; x += step) {
        const cx = Math.min(w, x);
        const pC = evalBone(bi, cx, cy);
        if (pi === -1 || !sock) {
          vx[cy * (w + 1) + cx] = pC.x;
          vy[cy * (w + 1) + cx] = pC.y;
        } else {
          const dSock = Math.hypot(cx - sock.x, cy - sock.y);
          if (dSock < blendR) {
            const pP = evalBone(pi, cx, cy);
            const t = dSock / blendR;
            const s = t * t * (3 - 2 * t);
            vx[cy * (w + 1) + cx] = pP.x * (1 - s) + pC.x * s;
            vy[cy * (w + 1) + cx] = pP.y * (1 - s) + pC.y * s;
          } else {
            vx[cy * (w + 1) + cx] = pC.x;
            vy[cy * (w + 1) + cx] = pC.y;
          }
        }
      }
    }
    for (let y = y0; y <= y1; y += step) {
      const yNext = Math.min(h, y + step);
      for (let x = x0; x <= x1; x += step) {
        const xNext = Math.min(w, x + step);
        let belongs = false;
        for (let ty = y; ty < yNext && !belongs; ty++) {
          for (let tx = x; tx < xNext; tx++) {
            const ci = ty * w + tx;
            if (grid[ci * 4 + 3] > 0 && geo.domPin[ci] === bi) { belongs = true; break; }
          }
        }
        if (!belongs) continue;
        const i00 = y * (w + 1) + x, i10 = y * (w + 1) + xNext;
        const i01 = yNext * (w + 1) + x, i11 = yNext * (w + 1) + xNext;
        rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vx[i00], vy[i00], x, y, vx[i10], vy[i10], xNext, y, vx[i11], vy[i11], xNext, yNext, depth);
        rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vx[i00], vy[i00], x, y, vx[i11], vy[i11], xNext, yNext, vx[i01], vy[i01], x, yNext, depth);
      }
    }
  }
  return alignAndSealOutput(out, rig, fx);
}

// --- METHOD 4 V3: PER-BONE 2ND-ORDER SPRING-DAMPER FOLLOW-THROUGH + TRACKED BLINK ---
function detectEyePixels(rig) {
  if (rig.eyeCache) return rig.eyeCache;
  const { grid, w, h, pins, bbox } = rig;
  const head = pins.find((p) => p.id === "head") || pins[0];
  const eyes = [];
  if (!head) { rig.eyeCache = eyes; return eyes; }
  const rX = Math.max(8, Math.round(bbox.bw * 0.16));
  const rY = Math.max(8, Math.round(bbox.bh * 0.16));
  const x0 = Math.max(1, Math.round(head.x - rX)), x1 = Math.min(w - 2, Math.round(head.x + rX));
  const y0 = Math.max(1, Math.round(head.y - rY)), y1 = Math.min(h - 2, Math.round(head.y + rY));

  for (let y = y0; y <= y1; y++) {
    for (let x = x0; x <= x1; x++) {
      const p = (y * w + x) * 4;
      if (grid[p + 3] === 0) continue;
      const lum = grid[p] * 0.299 + grid[p + 1] * 0.587 + grid[p + 2] * 0.114;
      if (lum > 75) continue;
      if (grid[(p - 4) + 3] > 0 && grid[(p + 4) + 3] > 0 && grid[(p - w * 4) + 3] > 0 && grid[(p + w * 4) + 3] > 0) {
        eyes.push({ x, y });
      }
    }
  }
  rig.eyeCache = eyes.length >= 2 && eyes.length <= 45 ? eyes : [];
  return rig.eyeCache;
}

function evalSpringCycle(fx, totalFrames, amp, k, c) {
  const N = Math.max(1, totalFrames);
  const subSteps = 4;
  const dt = 1.0 / (N * subSteps);
  let q = 0, v = 0;
  const out = new Float32Array(N);
  for (let step = 0; step < N * 3; step++) {
    for (let s = 0; s < subSteps; s++) {
      const ph = (((step % N) + s / subSteps) / N) * Math.PI * 2;
      const target = Math.sin(ph * 2) * amp;
      const acc = k * (target - q) - c * v;
      v += acc * dt;
      q += v * dt;
    }
    if (step >= N * 2) out[step - N * 2] = q;
  }
  return out;
}

function renderComplementaryDynFrame(rig, fx, frameIdx, totalFrames, ampScale) {
  const { grid, epx, w, h, bbox, pins } = rig;
  const amp = (typeof ampScale === "number" ? ampScale : 1.0) * 1.12;
  const world = computeBoneFkTransforms(rig, fx, frameIdx, totalFrames, amp);
  const weights = computeBiharmonicWeights(rig);

  // Per-bone secondary spring-damper lag on head & tail (Zhang et al. SIGGRAPH 2020)
  const lagCycle = evalSpringCycle(fx, totalFrames, 0.08, 18.0, 5.5);
  const lagAng = lagCycle[frameIdx % Math.max(1, totalFrames)];
  for (let i = 0; i < pins.length; i++) {
    const id = pins[i].id;
    if (id === "head" || id === "tail" || id === "ear_l" || id === "ear_r") {
      const baseAng = Math.atan2(world[i].sa, world[i].ca) + (id === "tail" ? lagAng * 1.6 : lagAng);
      world[i].ca = Math.cos(baseAng);
      world[i].sa = Math.sin(baseAng);
    }
  }

  const sqAmp = fx === "hop" ? 0.065 : (fx === "sprint" ? 0.042 : 0.024);
  const sq = evalSpringCycle(fx, totalFrames, sqAmp, 22.0, 6.5)[frameIdx % Math.max(1, totalFrames)];
  const sx = 1.0 + sq, sy = 1.0 / sx;

  const out = new Uint8ClampedArray(w * h * 4);
  const zBuf = new Int8Array(w * h).fill(-10);
  const ew = epx.w, eh = epx.h, edata = epx.data, efactor = epx.factor || 2;
  const step = Math.max(w, h) > 96 ? 2 : 1;
  const y0 = Math.max(0, bbox.minY), y1 = Math.min(h - 1, bbox.maxY);
  const x0 = Math.max(0, bbox.minX), x1 = Math.min(w - 1, bbox.maxX);

  const vx = new Float32Array((w + 1) * (h + 1));
  const vy = new Float32Array((w + 1) * (h + 1));
  evalBbwRigidMesh(rig, world, weights, vx, vy, step, sx, sy);

  for (let y = y0; y <= y1; y += step) {
    const yNext = Math.min(h, y + step);
    for (let x = x0; x <= x1; x += step) {
      const xNext = Math.min(w, x + step);
      let hasOpaque = false;
      for (let ty = y; ty < yNext && !hasOpaque; ty++) {
        for (let tx = x; tx < xNext; tx++) {
          if (grid[(ty * w + tx) * 4 + 3] > 0) { hasOpaque = true; break; }
        }
      }
      if (!hasOpaque) continue;
      const i00 = y * (w + 1) + x, i10 = y * (w + 1) + xNext;
      const i01 = yNext * (w + 1) + x, i11 = yNext * (w + 1) + xNext;
      rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vx[i00], vy[i00], x, y, vx[i10], vy[i10], xNext, y, vx[i11], vy[i11], xNext, yNext, 3);
      rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vx[i00], vy[i00], x, y, vx[i11], vy[i11], xNext, yNext, vx[i01], vy[i01], x, yNext, 3);
    }
  }

  const finalOut = alignAndSealOutput(out, rig, fx);

  const isBlink = (fx === "sleep") || (totalFrames >= 6 && frameIdx === Math.floor(totalFrames * 0.65));
  if (isBlink) {
    const eyes = detectEyePixels(rig);
    const headIdx = Math.max(0, pins.findIndex((p) => p.id === "head"));
    const ht = world[headIdx];
    for (let i = 0; i < eyes.length; i++) {
      const rx = eyes[i].x - ht.pivInX, ry = eyes[i].y - ht.pivInY;
      const ex = Math.round(ht.pivOutX + rx * ht.ca - ry * ht.sa);
      const ey = Math.round(ht.pivOutY + rx * ht.sa + ry * ht.ca);
      if (ex <= 1 || ex >= w - 1 || ey <= 1 || ey >= h - 1) continue;
      const p = (ey * w + ex) * 4;
      const pUp = ((ey - 1) * w + ex) * 4;
      if (finalOut[p + 3] > 0 && finalOut[pUp + 3] > 0) {
        finalOut[p] = finalOut[pUp]; finalOut[p + 1] = finalOut[pUp + 1]; finalOut[p + 2] = finalOut[pUp + 2];
      }
    }
  }
  return finalOut;
}


// ============================================================================
// ADDITIONAL SOTA RESEARCH METHODS (PAPERS 5-7) + UNIFIED SOTA HYBRID
// 5. arap_2d       : As-Rigid-As-Possible 2D Local-Global Solver (Igarashi 2005 / Sorkine 2007)
// 6. cleanedge_rot : Slope-Aware CleanEdge Pixel-Art Sampling (Kopf-Lischinski 2011 / CleanEdge)
// 7. parallax_25d  : 2.5D Cartoon Depth Parallax & Foreshortening (Rivers et al. SIGGRAPH 2010)
// 8. sota_hybrid   : Unified Mix-and-Match (BBW + FABRIK + Inpaint + ARAP + Spring Lag + Blink)
// ============================================================================

// --- PAPER 5: EXACT 2D ARAP LOCAL-GLOBAL SOLVER ---
function fitOptimalRotation2D(restPts, defPts) {
  let rcx = 0, rcy = 0, dcx = 0, dcy = 0;
  const n = restPts.length;
  for (let i = 0; i < n; i++) {
    rcx += restPts[i].x; rcy += restPts[i].y;
    dcx += defPts[i].x; dcy += defPts[i].y;
  }
  rcx /= n; rcy /= n; dcx /= n; dcy /= n;
  let sxx = 0, sxy = 0, syx = 0, syy = 0;
  for (let i = 0; i < n; i++) {
    const rx = restPts[i].x - rcx, ry = restPts[i].y - rcy;
    const dx = defPts[i].x - dcx, dy = defPts[i].y - dcy;
    sxx += rx * dx; sxy += rx * dy;
    syx += ry * dx; syy += ry * dy;
  }
  // Exact 2D Procrustes / polar-decomposition optimal rotation angle
  const theta = Math.atan2(sxy - syx, sxx + syy);
  return { theta, ca: Math.cos(theta), sa: Math.sin(theta) };
}

function runArapLocalGlobal2D(vx, vy, w, h, x0, y0, x1, y1, step, pinnedMask, iters) {
  const gw = w + 1;
  const cols = Math.ceil((x1 - x0) / step) + 1;
  const rows = Math.ceil((y1 - y0) / step) + 1;
  const cellCa = new Float32Array(cols * rows).fill(1);
  const cellSa = new Float32Array(cols * rows).fill(0);

  for (let it = 0; it < iters; it++) {
    // Local step: fit exact rigid rotation R_c for each quad cell
    for (let r = 0; r < rows - 1; r++) {
      const y = Math.min(h, y0 + r * step), yN = Math.min(h, y + step);
      for (let c = 0; c < cols - 1; c++) {
        const x = Math.min(w, x0 + c * step), xN = Math.min(w, x + step);
        const i00 = y * gw + x, i10 = y * gw + xN, i01 = yN * gw + x, i11 = yN * gw + xN;
        const rcx = (x + xN) * 0.5, rcy = (y + yN) * 0.5;
        const dcx = (vx[i00] + vx[i10] + vx[i01] + vx[i11]) * 0.25;
        const dcy = (vy[i00] + vy[i10] + vy[i01] + vy[i11]) * 0.25;
        const rx0 = x - rcx, rx1 = xN - rcx, ry0 = y - rcy, ry1 = yN - rcy;
        const sxx = rx0 * (vx[i00] - dcx) + rx1 * (vx[i10] - dcx) + rx0 * (vx[i01] - dcx) + rx1 * (vx[i11] - dcx);
        const sxy = rx0 * (vy[i00] - dcy) + rx1 * (vy[i10] - dcy) + rx0 * (vy[i01] - dcy) + rx1 * (vy[i11] - dcy);
        const syx = ry0 * (vx[i00] - dcx) + ry0 * (vx[i10] - dcx) + ry1 * (vx[i01] - dcx) + ry1 * (vx[i11] - dcx);
        const syy = ry0 * (vy[i00] - dcy) + ry0 * (vy[i10] - dcy) + ry1 * (vy[i01] - dcy) + ry1 * (vy[i11] - dcy);
        const theta = Math.atan2(sxy - syx, sxx + syy);
        const ci = r * cols + c;
        cellCa[ci] = Math.cos(theta);
        cellSa[ci] = Math.sin(theta);
      }
    }

    // Global step: relax free vertices toward their neighbors' rotated rest offsets
    for (let r = 1; r < rows - 1; r++) {
      const y = Math.min(h, y0 + r * step);
      for (let c = 1; c < cols - 1; c++) {
        const x = Math.min(w, x0 + c * step);
        const vi = y * gw + x;
        if (pinnedMask && pinnedMask[vi]) continue;
        const ci = r * cols + c;
        const ca = cellCa[ci], sa = cellSa[ci];
        const iL = y * gw + Math.max(0, x - step);
        const iR = y * gw + Math.min(w, x + step);
        const iU = Math.max(0, y - step) * gw + x;
        const iD = Math.min(h, y + step) * gw + x;
        const tx = 0.25 * (
          (vx[iL] + step * ca) + (vx[iR] - step * ca) +
          (vx[iU] - step * sa) + (vx[iD] + step * sa)
        );
        const ty = 0.25 * (
          (vy[iL] + step * sa) + (vy[iR] - step * sa) +
          (vy[iU] + step * ca) + (vy[iD] - step * ca)
        );
        vx[vi] = vx[vi] * 0.45 + tx * 0.55;
        vy[vi] = vy[vi] * 0.45 + ty * 0.55;
      }
    }
  }
}

function renderArap2dFrame(rig, fx, frameIdx, totalFrames, ampScale) {
  const { grid, epx, w, h, bbox, pins } = rig;
  const world = computeBoneFkTransforms(rig, fx, frameIdx, totalFrames, ampScale);
  const weights = computeBiharmonicWeights(rig);

  const out = new Uint8ClampedArray(w * h * 4);
  const zBuf = new Int8Array(w * h).fill(-10);
  const ew = epx.w, eh = epx.h, edata = epx.data, efactor = epx.factor || 2;
  const step = Math.max(w, h) > 96 ? 2 : 1;
  const y0 = Math.max(0, bbox.minY), y1 = Math.min(h - 1, bbox.maxY);
  const x0 = Math.max(0, bbox.minX), x1 = Math.min(w - 1, bbox.maxX);

  const vx = new Float32Array((w + 1) * (h + 1));
  const vy = new Float32Array((w + 1) * (h + 1));
  evalBbwRigidMesh(rig, world, weights, vx, vy, step, 1.0, 1.0);

  // Pin bone-handle vertices so ARAP preserves exact skeletal pose while smoothing joints rigidly
  const pinned = new Uint8Array((w + 1) * (h + 1));
  for (let i = 0; i < pins.length; i++) {
    const px = Math.max(x0, Math.min(x1, Math.round(pins[i].x / step) * step));
    const py = Math.max(y0, Math.min(y1, Math.round(pins[i].y / step) * step));
    pinned[py * (w + 1) + px] = 1;
  }
  runArapLocalGlobal2D(vx, vy, w, h, x0, y0, x1, y1, step, pinned, 3);

  for (let y = y0; y <= y1; y += step) {
    const yNext = Math.min(h, y + step);
    for (let x = x0; x <= x1; x += step) {
      const xNext = Math.min(w, x + step);
      let hasOpaque = false;
      for (let ty = y; ty < yNext && !hasOpaque; ty++) {
        for (let tx = x; tx < xNext; tx++) {
          if (grid[(ty * w + tx) * 4 + 3] > 0) { hasOpaque = true; break; }
        }
      }
      if (!hasOpaque) continue;
      const i00 = y * (w + 1) + x, i10 = y * (w + 1) + xNext;
      const i01 = yNext * (w + 1) + x, i11 = yNext * (w + 1) + xNext;
      rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vx[i00], vy[i00], x, y, vx[i10], vy[i10], xNext, y, vx[i11], vy[i11], xNext, yNext, 3);
      rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vx[i00], vy[i00], x, y, vx[i11], vy[i11], xNext, yNext, vx[i01], vy[i01], x, yNext, 3);
    }
  }
  return alignAndSealOutput(out, rig, fx);
}

// --- PAPER 6: CLEANEDGE / SLOPE-AWARE PIXEL-ART SAMPLING ---
function sampleCleanEdgePixel(grid, w, h, u, v) {
  const x0 = Math.floor(u), y0 = Math.floor(v);
  const fx = u - x0, fy = v - y0;
  const cx = Math.max(0, Math.min(w - 1, x0));
  const cy = Math.max(0, Math.min(h - 1, y0));
  const nx = Math.max(0, Math.min(w - 1, x0 + 1));
  const ny = Math.max(0, Math.min(h - 1, y0 + 1));

  const p00 = (cy * w + cx) * 4;
  const p10 = (cy * w + nx) * 4;
  const p01 = (ny * w + cx) * 4;
  const p11 = (ny * w + nx) * 4;

  const same = (a, b) =>
    grid[a + 3] === grid[b + 3] &&
    Math.abs(grid[a] - grid[b]) + Math.abs(grid[a + 1] - grid[b + 1]) + Math.abs(grid[a + 2] - grid[b + 2]) < 36;

  // CleanEdge diagonal resolution + checkerboard tie-breaker (connect opaque/darker ink diagonal)
  const mainSame = same(p00, p11);
  const antiSame = same(p10, p01);
  let diagMain = mainSame && !antiSame;
  let diagAnti = antiSame && !mainSame;
  if (mainSame && antiSame && !same(p00, p10)) {
    const lumM = grid[p00 + 3] === 0 ? 999 : (grid[p00] * 0.299 + grid[p00 + 1] * 0.587 + grid[p00 + 2] * 0.114);
    const lumA = grid[p10 + 3] === 0 ? 999 : (grid[p10] * 0.299 + grid[p10 + 1] * 0.587 + grid[p10 + 2] * 0.114);
    if (lumM <= lumA) diagMain = true;
    else diagAnti = true;
  }
  let pick = p00;
  if (diagMain) {
    if (Math.abs(fx - fy) < 0.45) pick = p00;
    else pick = fx > fy ? p10 : p01;
  } else if (diagAnti) {
    if (Math.abs(fx + fy - 1.0) < 0.45) pick = p10;
    else pick = (fx + fy < 1.0) ? p00 : p11;
  } else {
    pick = (fx < 0.5) ? (fy < 0.5 ? p00 : p01) : (fy < 0.5 ? p10 : p11);
  }
  return pick;
}

function rasterizeTriCleanEdge(out, zBuf, w, h, grid, x0, y0, u0, v0, x1, y1, u1, v1, x2, y2, u2, v2, depth) {
  const minX = Math.max(0, Math.floor(Math.min(x0, x1, x2)));
  const maxX = Math.min(w - 1, Math.ceil(Math.max(x0, x1, x2)));
  const minY = Math.max(0, Math.floor(Math.min(y0, y1, y2)));
  const maxY = Math.min(h - 1, Math.ceil(Math.max(y0, y1, y2)));
  const area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
  if (Math.abs(area) < 1e-4) return;
  const invA = 1.0 / area;

  for (let y = minY; y <= maxY; y++) {
    const py = y + 0.5;
    for (let x = minX; x <= maxX; x++) {
      const px = x + 0.5;
      const w0 = ((x1 - px) * (y2 - py) - (x2 - px) * (y1 - py)) * invA;
      const w1 = ((x2 - px) * (y0 - py) - (x0 - px) * (y2 - py)) * invA;
      const w2 = 1.0 - w0 - w1;
      if (w0 < -0.002 || w1 < -0.002 || w2 < -0.002) continue;
      const idx = y * w + x;
      if (depth < zBuf[idx]) continue;
      const u = w0 * u0 + w1 * u1 + w2 * u2;
      const v = w0 * v0 + w1 * v1 + w2 * v2;
      const sp = sampleCleanEdgePixel(grid, w, h, u, v);
      if (grid[sp + 3] === 0) continue;
      zBuf[idx] = depth;
      const dp = idx * 4;
      out[dp] = grid[sp]; out[dp + 1] = grid[sp + 1]; out[dp + 2] = grid[sp + 2]; out[dp + 3] = 255;
    }
  }
}

function renderCleanEdgeFrame(rig, fx, frameIdx, totalFrames, ampScale) {
  const { grid, w, h, bbox } = rig;
  const world = computeBoneFkTransforms(rig, fx, frameIdx, totalFrames, ampScale);
  const weights = computeBiharmonicWeights(rig);

  const out = new Uint8ClampedArray(w * h * 4);
  const zBuf = new Int8Array(w * h).fill(-10);
  const step = Math.max(w, h) > 96 ? 2 : 1;
  const y0 = Math.max(0, bbox.minY), y1 = Math.min(h - 1, bbox.maxY);
  const x0 = Math.max(0, bbox.minX), x1 = Math.min(w - 1, bbox.maxX);

  const vx = new Float32Array((w + 1) * (h + 1));
  const vy = new Float32Array((w + 1) * (h + 1));
  evalBbwRigidMesh(rig, world, weights, vx, vy, step, 1.0, 1.0);

  for (let y = y0; y <= y1; y += step) {
    const yNext = Math.min(h, y + step);
    for (let x = x0; x <= x1; x += step) {
      const xNext = Math.min(w, x + step);
      let hasOpaque = false;
      for (let ty = y; ty < yNext && !hasOpaque; ty++) {
        for (let tx = x; tx < xNext; tx++) {
          if (grid[(ty * w + tx) * 4 + 3] > 0) { hasOpaque = true; break; }
        }
      }
      if (!hasOpaque) continue;
      const i00 = y * (w + 1) + x, i10 = y * (w + 1) + xNext;
      const i01 = yNext * (w + 1) + x, i11 = yNext * (w + 1) + xNext;
      rasterizeTriCleanEdge(out, zBuf, w, h, grid, vx[i00], vy[i00], x, y, vx[i10], vy[i10], xNext, y, vx[i11], vy[i11], xNext, yNext, 3);
      rasterizeTriCleanEdge(out, zBuf, w, h, grid, vx[i00], vy[i00], x, y, vx[i11], vy[i11], xNext, yNext, vx[i01], vy[i01], x, yNext, 3);
    }
  }
  return alignAndSealOutput(out, rig, fx);
}

// --- PAPER 7: 2.5D CARTOON MODELS (RIVERS ET AL., SIGGRAPH 2010) ---
function applyParallax25DTransforms(pins, world, bbox, phase, fx, bodyType) {
  let yawAmp = fx === "dance" ? 0.15 : (fx === "walk" || fx === "sprint" ? 0.09 : 0.06);
  if (bodyType === "blob" || bodyType === "crawler") yawAmp *= 0.35;
  const yaw = Math.sin(phase) * yawAmp;
  const sinY = Math.sin(yaw);
  const cosY = Math.cos(yaw);
  const cx = (bbox.minX + bbox.maxX) * 0.5;
  const depthScale = Math.max(8, bbox.bw * 0.14);

  for (let i = 0; i < pins.length; i++) {
    const id = pins[i].id;
    let z = 0;
    if (id === "head" || id === "ear_l" || id === "ear_r") z = 0.65;
    else if (id === "fl" || id === "bl" || id === "l_hand") z = 0.45;
    else if (id === "fr" || id === "br" || id === "r_hand") z = -0.45;
    else if (id === "tail") z = -0.35;
    // Foreshorten X around character center + shift by z * sin(yaw)
    const dx = world[i].pivOutX - cx;
    world[i].pivOutX = cx + dx * cosY + z * depthScale * sinY;
  }
}

function renderParallax25dFrame(rig, fx, frameIdx, totalFrames, ampScale) {
  const { grid, epx, w, h, bbox, pins } = rig;
  const world = computeBoneFkTransforms(rig, fx, frameIdx, totalFrames, ampScale);
  const phase = (frameIdx / Math.max(1, totalFrames)) * Math.PI * 2;
  applyParallax25DTransforms(pins, world, bbox, phase, fx, rig.bodyType);
  const weights = computeBiharmonicWeights(rig);

  const out = new Uint8ClampedArray(w * h * 4);
  const zBuf = new Int8Array(w * h).fill(-10);
  const ew = epx.w, eh = epx.h, edata = epx.data, efactor = epx.factor || 2;
  const step = Math.max(w, h) > 96 ? 2 : 1;
  const y0 = Math.max(0, bbox.minY), y1 = Math.min(h - 1, bbox.maxY);
  const x0 = Math.max(0, bbox.minX), x1 = Math.min(w - 1, bbox.maxX);

  const vx = new Float32Array((w + 1) * (h + 1));
  const vy = new Float32Array((w + 1) * (h + 1));
  evalBbwRigidMesh(rig, world, weights, vx, vy, step, 1.0, 1.0);

  for (let y = y0; y <= y1; y += step) {
    const yNext = Math.min(h, y + step);
    for (let x = x0; x <= x1; x += step) {
      const xNext = Math.min(w, x + step);
      let hasOpaque = false;
      for (let ty = y; ty < yNext && !hasOpaque; ty++) {
        for (let tx = x; tx < xNext; tx++) {
          if (grid[(ty * w + tx) * 4 + 3] > 0) { hasOpaque = true; break; }
        }
      }
      if (!hasOpaque) continue;
      const i00 = y * (w + 1) + x, i10 = y * (w + 1) + xNext;
      const i01 = yNext * (w + 1) + x, i11 = yNext * (w + 1) + xNext;
      rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vx[i00], vy[i00], x, y, vx[i10], vy[i10], xNext, y, vx[i11], vy[i11], xNext, yNext, 3);
      rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vx[i00], vy[i00], x, y, vx[i11], vy[i11], xNext, yNext, vx[i01], vy[i01], x, yNext, 3);
    }
  }
  return alignAndSealOutput(out, rig, fx);
}

// --- METHOD 8: SOTA HYBRID (MIX-AND-MATCH: BBW + FABRIK + ARAP + SPRING LAG + 2.5D + BLINK) ---
function renderSotaHybridFrame(rig, fx, frameIdx, totalFrames, ampScale) {
  const { grid, w, h, bbox, pins } = rig;
  const amp = (typeof ampScale === "number" ? ampScale : 1.0) * 1.08;
  const world = computeBoneFkTransforms(rig, fx, frameIdx, totalFrames, amp);
  const phase = (frameIdx / Math.max(1, totalFrames)) * Math.PI * 2;

  // 1. Apply 2.5D Cartoon Parallax
  applyParallax25DTransforms(pins, world, bbox, phase, fx, rig.bodyType);

  // 2. Apply Per-Bone 2nd-Order Spring-Damper Lag (Complementary Dynamics)
  const lagCycle = evalSpringCycle(fx, totalFrames, 0.075, 18.0, 5.5);
  const lagAng = lagCycle[frameIdx % Math.max(1, totalFrames)];
  for (let i = 0; i < pins.length; i++) {
    const id = pins[i].id;
    if (id === "head" || id === "tail" || id === "ear_l" || id === "ear_r") {
      const baseAng = Math.atan2(world[i].sa, world[i].ca) + (id === "tail" ? lagAng * 1.5 : lagAng);
      world[i].ca = Math.cos(baseAng);
      world[i].sa = Math.sin(baseAng);
    }
  }

  // 3. Evaluate BBW + Spring Squash-and-Stretch
  const weights = computeBiharmonicWeights(rig);
  const sqAmp = fx === "hop" ? 0.055 : (fx === "sprint" ? 0.036 : 0.02);
  const sq = evalSpringCycle(fx, totalFrames, sqAmp, 22.0, 6.5)[frameIdx % Math.max(1, totalFrames)];
  const sx = 1.0 + sq, sy = 1.0 / sx;

  const step = Math.max(w, h) > 96 ? 2 : 1;
  const y0 = Math.max(0, bbox.minY), y1 = Math.min(h - 1, bbox.maxY);
  const x0 = Math.max(0, bbox.minX), x1 = Math.min(w - 1, bbox.maxX);
  const vx = new Float32Array((w + 1) * (h + 1));
  const vy = new Float32Array((w + 1) * (h + 1));
  evalBbwRigidMesh(rig, world, weights, vx, vy, step, sx, sy);

  // 4. ARAP 2D Local-Global Rigidity Pass
  const pinned = new Uint8Array((w + 1) * (h + 1));
  for (let i = 0; i < pins.length; i++) {
    const px = Math.max(x0, Math.min(x1, Math.round(pins[i].x / step) * step));
    const py = Math.max(y0, Math.min(y1, Math.round(pins[i].y / step) * step));
    pinned[py * (w + 1) + px] = 1;
  }
  runArapLocalGlobal2D(vx, vy, w, h, x0, y0, x1, y1, step, pinned, 2);

  // 5. CleanEdge Slope-Aware Rasterization
  const out = new Uint8ClampedArray(w * h * 4);
  const zBuf = new Int8Array(w * h).fill(-10);
  for (let y = y0; y <= y1; y += step) {
    const yNext = Math.min(h, y + step);
    for (let x = x0; x <= x1; x += step) {
      const xNext = Math.min(w, x + step);
      let hasOpaque = false;
      for (let ty = y; ty < yNext && !hasOpaque; ty++) {
        for (let tx = x; tx < xNext; tx++) {
          if (grid[(ty * w + tx) * 4 + 3] > 0) { hasOpaque = true; break; }
        }
      }
      if (!hasOpaque) continue;
      const i00 = y * (w + 1) + x, i10 = y * (w + 1) + xNext;
      const i01 = yNext * (w + 1) + x, i11 = yNext * (w + 1) + xNext;
      rasterizeTriCleanEdge(out, zBuf, w, h, grid, vx[i00], vy[i00], x, y, vx[i10], vy[i10], xNext, y, vx[i11], vy[i11], xNext, yNext, 3);
      rasterizeTriCleanEdge(out, zBuf, w, h, grid, vx[i00], vy[i00], x, y, vx[i11], vy[i11], xNext, yNext, vx[i01], vy[i01], x, yNext, 3);
    }
  }

  const finalOut = alignAndSealOutput(out, rig, fx);

  // 6. Tracked Auto-Blink
  const isBlink = (fx === "sleep") || (totalFrames >= 6 && frameIdx === Math.floor(totalFrames * 0.65));
  if (isBlink) {
    const eyes = detectEyePixels(rig);
    const headIdx = Math.max(0, pins.findIndex((p) => p.id === "head"));
    const ht = world[headIdx];
    for (let i = 0; i < eyes.length; i++) {
      const rx = eyes[i].x - ht.pivInX, ry = eyes[i].y - ht.pivInY;
      const ex = Math.round(ht.pivOutX + rx * ht.ca - ry * ht.sa);
      const ey = Math.round(ht.pivOutY + rx * ht.sa + ry * ht.ca);
      if (ex <= 1 || ex >= w - 1 || ey <= 1 || ey >= h - 1) continue;
      const p = (ey * w + ex) * 4;
      const pUp = ((ey - 1) * w + ex) * 4;
      if (finalOut[p + 3] > 0 && finalOut[pUp + 3] > 0) {
        finalOut[p] = finalOut[pUp]; finalOut[p + 1] = finalOut[pUp + 1]; finalOut[p + 2] = finalOut[pUp + 2];
      }
    }
  }
  return finalOut;
}

function renderRiggedFrame(rig, fx, frameIdx, totalFrames, techOpt) {
  const { grid, epx, w, h, bbox, pins, geo } = rig;
  let ampScale = 1.0;
  if (typeof document !== "undefined") {
    const ampEl = document.getElementById("gif-amp");
    if (ampEl) ampScale = Math.max(0.3, Math.min(2.0, parseFloat(ampEl.value) || 1.0));
  }

  const dstPins = computeAnimatedPinsWithSpring(rig, fx, frameIdx, totalFrames, ampScale);
  const out = new Uint8ClampedArray(w * h * 4);
  const zBuf = new Int8Array(w * h).fill(-10);
  const ew = epx.w, eh = epx.h, edata = epx.data, efactor = epx.factor || 2;

  const pinDepth = new Int8Array(pins.length);
  const pinIds = pins.map((p) => p.id);
  const isLimbPin = new Uint8Array(pins.length);
  for (let i = 0; i < pins.length; i++) {
    const id = pinIds[i];
    if (id === "fr" || id === "bl") { pinDepth[i] = 1; isLimbPin[i] = 1; }
    else if (id === "tail") pinDepth[i] = 2;
    else if (id === "chest" || id === "pelvis") pinDepth[i] = 3;
    else if (id === "head") pinDepth[i] = 4;
    else if (id === "wing" || id === "l_hand") pinDepth[i] = 6;
    else if (id === "fl" || id === "br") { pinDepth[i] = 5; isLimbPin[i] = 2; }
    else pinDepth[i] = 5;
  }

  let tech = techOpt;
  if (!tech && typeof document !== "undefined") {
    const tEl = document.getElementById("gif-tech-select");
    tech = tEl ? tEl.value : "bone_fk";
  }
  if (!tech || tech === "bone_fk") {
    const res = renderBoneFkFrame(rig, fx, frameIdx, totalFrames, ampScale);
    sealPinholesAndSpecks(res, rig.w, rig.h);
    return res;
  }
  if (tech === "harmonic_dqs") {
    return renderHarmonicDqsFrame(rig, fx, frameIdx, totalFrames, ampScale);
  }
  if (tech === "fabrik_ik") {
    return renderFabrikIkFrame(rig, fx, frameIdx, totalFrames, ampScale);
  }
  if (tech === "layered_inpaint") {
    return renderLayeredInpaintFrame(rig, fx, frameIdx, totalFrames, ampScale);
  }
  if (tech === "complementary_dyn") {
    return renderComplementaryDynFrame(rig, fx, frameIdx, totalFrames, ampScale);
  }
  if (tech === "arap_2d") {
    return renderArap2dFrame(rig, fx, frameIdx, totalFrames, ampScale);
  }
  if (tech === "cleanedge_rot") {
    return renderCleanEdgeFrame(rig, fx, frameIdx, totalFrames, ampScale);
  }
  if (tech === "parallax_25d") {
    return renderParallax25dFrame(rig, fx, frameIdx, totalFrames, ampScale);
  }
  if (tech === "sota_hybrid") {
    return renderSotaHybridFrame(rig, fx, frameIdx, totalFrames, ampScale);
  }
  const useArticulatedLimbs =
    tech !== "geodesic_mls" &&
    (rig.bodyType === "quadruped" || rig.bodyType === "biped") &&
    (fx === "walk" || fx === "sprint" || fx === "run" || fx === "dance" || fx === "hop");

  const step = Math.max(w, h) > 96 ? 2 : 1;
  const y0 = Math.max(0, bbox.minY), y1 = Math.min(h - 1, bbox.maxY);
  const x0 = Math.max(0, bbox.minX), x1 = Math.min(w - 1, bbox.maxX);

  const bodySrcPins = [];
  const bodyDstPins = [];
  const bodyPinMap = [];
  if (useArticulatedLimbs) {
    for (let i = 0; i < pins.length; i++) {
      if (isLimbPin[i] === 0) {
        bodySrcPins.push(pins[i]);
        bodyDstPins.push(dstPins[i]);
        bodyPinMap.push(i);
      }
    }
  }
  const useTorsoOnly = useArticulatedLimbs && bodySrcPins.length >= 2;
  const bodyWRow = new Float32Array(Math.max(1, bodySrcPins.length));

  const vxBody = new Float32Array((w + 1) * (h + 1));
  const vyBody = new Float32Array((w + 1) * (h + 1));
  for (let y = y0; y <= y1 + step; y += step) {
    const cy = Math.min(h, y), sy = Math.min(h - 1, cy);
    for (let x = x0; x <= x1 + step; x += step) {
      const cx = Math.min(w, x), sx = Math.min(w - 1, cx);
      const cellIdx = sy * w + sx;
      let p;
      if (useTorsoOnly) {
        let sum = 0;
        for (let k = 0; k < bodyPinMap.length; k++) {
          const wk = geo.weights[cellIdx * geo.numPins + bodyPinMap[k]] + 1e-6;
          bodyWRow[k] = wk;
          sum += wk;
        }
        const inv = 1 / sum;
        for (let k = 0; k < bodyPinMap.length; k++) bodyWRow[k] *= inv;
        p = evalRigidMlsForward(cx, cy, bodySrcPins, bodyDstPins, bodyWRow, 0);
      } else {
        p = evalRigidMlsForward(cx, cy, pins, dstPins, geo.weights, cellIdx * geo.numPins);
      }
      const vi = cy * (w + 1) + cx;
      vxBody[vi] = p.x;
      vyBody[vi] = p.y;
    }
  }

  if (!useArticulatedLimbs) {
    for (let y = y0; y <= y1; y += step) {
      const yNext = Math.min(h, y + step);
      for (let x = x0; x <= x1; x += step) {
        const xNext = Math.min(w, x + step);
        let hasOpaque = false, sampleIdx = y * w + x;
        for (let ty = y; ty < yNext && !hasOpaque; ty++) {
          for (let tx = x; tx < xNext; tx++) {
            const ci = ty * w + tx;
            if (grid[ci * 4 + 3] > 0) { hasOpaque = true; sampleIdx = ci; break; }
          }
        }
        if (!hasOpaque) continue;
        const depth = pinDepth[geo.domPin[sampleIdx]];
        const i00 = y * (w + 1) + x, i10 = y * (w + 1) + xNext;
        const i01 = yNext * (w + 1) + x, i11 = yNext * (w + 1) + xNext;
        rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vxBody[i00], vyBody[i00], x, y, vxBody[i10], vyBody[i10], xNext, y, vxBody[i11], vyBody[i11], xNext, yNext, depth);
        rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vxBody[i00], vyBody[i00], x, y, vxBody[i11], vyBody[i11], xNext, yNext, vxBody[i01], vyBody[i01], x, yNext, depth);
      }
    }
  } else {
    const limbPoses = computeLimbGaitPose(rig, fx, frameIdx, totalFrames, ampScale);
    const hipAnchorY = Math.max(bbox.minY + 4, Math.min(rig.hipY - 2, bbox.maxY - Math.max(6, Math.round(bbox.bh * 0.22))));
    const blendZone = 8.0; // 4px joint blend at hip, then 100% rigid bone rotation down to paw!

    const drawLimbPass = (targetPinIdx, depth) => {
      const id = pinIds[targetPinIdx];
      const pose = limbPoses[id];
      const pin = pins[targetPinIdx];
      const vxL = new Float32Array((w + 1) * (h + 1));
      const vyL = new Float32Array((w + 1) * (h + 1));
      const yStart = y0 + Math.max(0, Math.floor((hipAnchorY - step * 2 - y0) / step)) * step;

      const sockX = Math.min(x1, x0 + Math.max(0, Math.round((pin.x - x0) / step)) * step);
      const sockY = Math.min(y1, y0 + Math.max(0, Math.round((hipAnchorY - y0) / step)) * step);
      const sockVi = sockY * (w + 1) + sockX;
      const pivX = vxBody[sockVi];
      const pivY = vyBody[sockVi];

      const fullAng = pose ? pose.angle : 0;
      const fullLift = pose ? pose.liftY : 0;

      for (let y = yStart; y <= y1 + step; y += step) {
        const cy = Math.min(h, y);
        // Linear 4px blend at hip socket, then constant 1.0 rigid rotation for the entire lower leg & paw
        const blend = Math.max(0, Math.min(1, (cy - (hipAnchorY + 2)) / blendZone));
        const ang = fullAng * blend;
        const ca = Math.cos(ang), sa = Math.sin(ang);
        const lY = fullLift * blend;
        for (let x = x0; x <= x1 + step; x += step) {
          const cx = Math.min(w, x);
          const vi = cy * (w + 1) + cx;
          const bx = vxBody[vi], by = vyBody[vi];
          if (blend <= 1e-4) {
            vxL[vi] = bx;
            vyL[vi] = by;
          } else {
            const rx = bx - pivX, ry = by - pivY;
            vxL[vi] = pivX + rx * ca - ry * sa;
            vyL[vi] = pivY + rx * sa + ry * ca + lY;
          }
        }
      }

      for (let y = yStart; y <= y1; y += step) {
        const yNext = Math.min(h, y + step);
        for (let x = x0; x <= x1; x += step) {
          const xNext = Math.min(w, x + step);
          let belongs = false;
          for (let ty = y; ty < yNext && !belongs; ty++) {
            for (let tx = x; tx < xNext; tx++) {
              const ci = ty * w + tx;
              if (grid[ci * 4 + 3] > 0 && ty >= hipAnchorY - step * 2) {
                if (geo.domPin[ci] === targetPinIdx) {
                  belongs = true;
                  break;
                }
                // 1-step overlap collar at limb/body & limb/limb seam prevents hairline white slits
                const lx = Math.max(0, tx - step), rx = Math.min(w - 1, tx + step);
                const uy = Math.max(0, ty - step), dy = Math.min(h - 1, ty + step);
                if (
                  geo.domPin[ty * w + lx] === targetPinIdx ||
                  geo.domPin[ty * w + rx] === targetPinIdx ||
                  geo.domPin[uy * w + tx] === targetPinIdx ||
                  geo.domPin[dy * w + tx] === targetPinIdx
                ) {
                  belongs = true;
                  break;
                }
              }
            }
          }
          if (!belongs) continue;
          const i00 = y * (w + 1) + x, i10 = y * (w + 1) + xNext;
          const i01 = yNext * (w + 1) + x, i11 = yNext * (w + 1) + xNext;
          rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vxL[i00], vyL[i00], x, y, vxL[i10], vyL[i10], xNext, y, vxL[i11], vyL[i11], xNext, yNext, depth);
          rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vxL[i00], vyL[i00], x, y, vxL[i11], vyL[i11], xNext, yNext, vxL[i01], vyL[i01], x, yNext, depth);
        }
      }
    };

    // Pass 1: Far limbs (fr, bl)
    for (let pi = 0; pi < pins.length; pi++) {
      if (isLimbPin[pi] === 1) drawLimbPass(pi, 1);
    }

    // Pass 2: Full Torso & Upper Hip Sockets
    for (let y = y0; y <= y1; y += step) {
      const yNext = Math.min(h, y + step);
      for (let x = x0; x <= x1; x += step) {
        const xNext = Math.min(w, x + step);
        let hasBody = false, sampleIdx = y * w + x;
        for (let ty = y; ty < yNext && !hasBody; ty++) {
          for (let tx = x; tx < xNext; tx++) {
            const ci = ty * w + tx;
            if (grid[ci * 4 + 3] > 0) {
              const dp = geo.domPin[ci];
              if (isLimbPin[dp] === 0 || ty <= hipAnchorY + Math.max(6, step * 3)) {
                hasBody = true;
                sampleIdx = ci;
                break;
              }
            }
          }
        }
        if (!hasBody) continue;
        const depth = Math.max(2, Math.min(4, pinDepth[geo.domPin[sampleIdx]]));
        const i00 = y * (w + 1) + x, i10 = y * (w + 1) + xNext;
        const i01 = yNext * (w + 1) + x, i11 = yNext * (w + 1) + xNext;
        rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vxBody[i00], vyBody[i00], x, y, vxBody[i10], vyBody[i10], xNext, y, vxBody[i11], vyBody[i11], xNext, yNext, depth);
        rasterizeTriEpx(out, zBuf, w, h, edata, ew, eh, efactor, vxBody[i00], vyBody[i00], x, y, vxBody[i11], vyBody[i11], xNext, yNext, vxBody[i01], vyBody[i01], x, yNext, depth);
      }
    }

    // Pass 3: Near limbs (fl, br) cleanly in front
    for (let pi = 0; pi < pins.length; pi++) {
      if (isLimbPin[pi] === 2) drawLimbPass(pi, 5);
    }
  }

  // Strict ground-baseline alignment for grounded cycles
  let finalOut = out;
  if ((fx === "walk" || fx === "breathe" || fx === "wave" || fx === "sleep") && rig.bodyType !== "bird" && rig.bodyType !== "swimmer") {
    let curFoot = -1;
    for (let y = h - 1; y >= 0 && curFoot === -1; y--) {
      for (let x = 0; x < w; x++) {
        if (out[(y * w + x) * 4 + 3] > 0) { curFoot = y; break; }
      }
    }
    if (curFoot !== -1 && curFoot !== bbox.maxY) {
      const shift = bbox.maxY - curFoot;
      if (Math.abs(shift) <= 12) {
        const aligned = new Uint8ClampedArray(w * h * 4);
        for (let y = 0; y < h; y++) {
          const ty = y + shift;
          if (ty < 0 || ty >= h) continue;
          for (let x = 0; x < w; x++) {
            const sp = (y * w + x) * 4, dp = (ty * w + x) * 4;
            aligned[dp] = out[sp]; aligned[dp + 1] = out[sp + 1]; aligned[dp + 2] = out[sp + 2]; aligned[dp + 3] = out[sp + 3];
          }
        }
        finalOut = aligned;
      }
    }
  }

  sealPinholesAndSpecks(finalOut, w, h);
  const lockEl = typeof document !== "undefined" ? document.getElementById("gif-outline-lock") : null;
  if ((!lockEl || lockEl.checked) && !rig.isHiResNative) {
    applyCrispOutlineLock(finalOut, w, h, rig.inkColor || [20, 22, 28]);
  }
  return finalOut;
}

function toggleGifPlay() {
  gifPaused = !gifPaused;
  const b = document.getElementById("gif-play-btn");
  if (b) b.textContent = gifPaused ? "PLAY" : "PAUSE";
}

function stepGifFrame(delta) {
  gifPaused = true;
  const b = document.getElementById("gif-play-btn");
  if (b) b.textContent = "PLAY";
  const prevC = document.getElementById("gif-preview-canvas");
  if (!prevC) return;
  const fx = document.getElementById("gif-fx-select").value;
  const total = getGifTotalFrames(fx);
  gifPreviewTick = (gifPreviewTick + delta + total) % total;
  drawGifMakerFrame(prevC.getContext("2d"), prevC.width, prevC.height, gifPreviewTick, total, true);
}

function resetGifPins() {
  if (!gifCustomRawImg) return;
  refreshCleanedCustomImg();
  if (typeof showToast === "function") showToast("Reset skeleton pins to auto-detected defaults");
}

function setSheetControlsVisible(show) {
  const row = document.getElementById("gif-sheet-controls");
  if (row) row.style.display = show ? "flex" : "none";
}

function refreshCleanedCustomImg() {
  if (!gifCustomRawImg || typeof document === "undefined") return;
  const w = gifCustomRawImg.naturalWidth || gifCustomRawImg.width || 128;
  const h = gifCustomRawImg.naturalHeight || gifCustomRawImg.height || 128;
  const c = document.createElement("canvas");
  c.width = w; c.height = h;
  const g = c.getContext("2d", { willReadFrequently: true });
  g.imageSmoothingEnabled = false;
  g.drawImage(gifCustomRawImg, 0, 0, w, h);
  const raw = g.getImageData(0, 0, w, h).data;

  const cleanChk = document.getElementById("gif-clean-bg");
  const cleanBg = !cleanChk || cleanChk.checked;
  const modeSel = document.getElementById("gif-mode-select");
  const mode = modeSel ? modeSel.value : "auto";
  const colsEl = document.getElementById("gif-sheet-cols");
  const rowsEl = document.getElementById("gif-sheet-rows");
  const framesEl = document.getElementById("gif-sheet-frames");
  const st = document.getElementById("gif-status");

  let useSheet = false;
  let cols = colsEl ? Math.max(1, parseInt(colsEl.value, 10) || 1) : 1;
  let rows = rowsEl ? Math.max(1, parseInt(rowsEl.value, 10) || 1) : 1;

  if (mode === "sheet") {
    useSheet = true;
    if (cols * rows <= 1) {
      const autoG = detectSpriteSheetGrid(raw, w, h);
      cols = autoG.isSheet ? autoG.cols : Math.max(2, Math.round(w / Math.max(1, h)));
      rows = autoG.isSheet ? autoG.rows : 1;
      if (colsEl) colsEl.value = String(cols);
      if (rowsEl) rowsEl.value = String(rows);
    }
  } else if (mode === "auto") {
    const autoG = detectSpriteSheetGrid(raw, w, h);
    if (autoG.isSheet) {
      useSheet = true;
      cols = autoG.cols; rows = autoG.rows;
      if (colsEl) colsEl.value = String(cols);
      if (rowsEl) rowsEl.value = String(rows);
    }
  }

  if (useSheet) {
    gifCustomSheet = sliceSpriteSheet(raw, w, h, cols, rows, cleanBg, framesEl ? framesEl.value : "all");
    gifCustomRig = null;
    setSheetControlsVisible(true);
    if (st && gifCustomSheet) {
      st.textContent = "SPRITE SHEET (" + cols + "x" + rows + ", " + gifCustomSheet.activeIndices.length + "/" + gifCustomSheet.frames.length + " FRAMES)";
    }
  } else {
    gifCustomSheet = null;
    setSheetControlsVisible(false);
    const bodySel = document.getElementById("gif-body-select");
    const tolEl = document.getElementById("gif-key-tol");
    const defEl = document.getElementById("gif-defringe");
    const cleanOpts = {
      keyColor: gifChromaKeyRgb,
      tolerance: tolEl ? (parseInt(tolEl.value, 10) || 40) : 40,
      defringe: defEl ? parseInt(defEl.value, 10) : 2,
    };
    invalidateGifFrameCache();
    gifCustomRig = buildAutoRig(raw, w, h, cleanBg, bodySel ? bodySel.value : "auto", cleanOpts);
    if (st) {
      const modeTag = gifCustomRig.isHiResNative ? "NATIVE 1:1 RIGID MLS" : "2x EPX RIGID MLS";
      st.textContent = modeTag + " (" + gifCustomRig.bodyType.toUpperCase() + ", " + gifCustomRig.nativeW + "x" + gifCustomRig.nativeH + ")";
    }
  }
}

function syncGifMascotSelection() {
  gifCustomRig = null; gifCustomSheet = null; gifCustomRawImg = null; gifCustomName = "";
  setSheetControlsVisible(false);
  const sel = document.getElementById("gif-pet-select");
  const cap = document.getElementById("gif-caption");
  const clr = document.getElementById("gif-clear-btn");
  if (clr) clr.style.display = "none";
  if (sel && cap) {
    const opt = sel.options[sel.selectedIndex];
    cap.value = opt ? opt.textContent.trim() : sel.value;
  }
  const st = document.getElementById("gif-status");
  if (st) st.textContent = "IN PROGRESS \u00B7 GIF89A & PNG SHEET";
}

function loadGifSpriteFile(file) {
  if (!file) return;
  const cleanName = file.name.replace(/\.[^.]+$/, "").replace(/[^a-zA-Z0-9_-]+/g, "_").slice(0, 24) || "custom_pet";

  if (file.name.toLowerCase().endsWith(".xml")) {
    const reader = new FileReader();
    reader.onload = () => {
      try {
        const doc = new DOMParser().parseFromString(String(reader.result || ""), "text/xml");
        const tx = parseInt((doc.querySelector("TilesX") && doc.querySelector("TilesX").textContent) || "1", 10) || 1;
        const ty = parseInt((doc.querySelector("TilesY") && doc.querySelector("TilesY").textContent) || "1", 10) || 1;
        const b64 = ((doc.querySelector("Png") && doc.querySelector("Png").textContent) || "").replace(/\s+/g, "");
        const titleEl = doc.querySelector("Header > Title");
        const title = (titleEl && titleEl.textContent.trim()) || cleanName;
        const walkFrames = [];
        doc.querySelectorAll('Animation[id="1"] Frame').forEach((f) => {
          const n = parseInt(f.textContent.trim(), 10);
          if (!isNaN(n)) walkFrames.push(n);
        });
        const img = new Image();
        img.onload = () => {
          gifCustomRawImg = img;
          gifCustomName = title.toLowerCase().replace(/[^a-z0-9_-]+/g, "_");
          const modeSel = document.getElementById("gif-mode-select");
          if (modeSel) modeSel.value = "sheet";
          const cEl = document.getElementById("gif-sheet-cols");
          const rEl = document.getElementById("gif-sheet-rows");
          const fEl = document.getElementById("gif-sheet-frames");
          if (cEl) cEl.value = String(tx);
          if (rEl) rEl.value = String(ty);
          if (fEl) fEl.value = walkFrames.length > 0 ? walkFrames.join(",") : "all";
          const cap = document.getElementById("gif-caption");
          if (cap) cap.value = title.slice(0, 18);
          const clr = document.getElementById("gif-clear-btn");
          if (clr) clr.style.display = "inline-block";
          refreshCleanedCustomImg();
          if (typeof showToast === "function") showToast("Loaded deskpet.xml: " + title);
        };
        img.src = "data:image/png;base64," + b64;
      } catch (e) {
        if (typeof showToast === "function") showToast("Invalid deskpet.xml file");
      }
    };
    reader.readAsText(file);
    return;
  }

  const applyLoaded = (img) => {
    gifCustomRawImg = img;
    gifCustomName = cleanName;
    const fEl = document.getElementById("gif-sheet-frames");
    if (fEl) fEl.value = "all";
    refreshCleanedCustomImg();
    const cap = document.getElementById("gif-caption");
    if (cap) cap.value = cleanName.replace(/_/g, " ").slice(0, 18);
    const clr = document.getElementById("gif-clear-btn");
    if (clr) clr.style.display = "inline-block";
    if (typeof showToast === "function") showToast("Loaded: " + file.name);
  };

  const isSvg = file.type === "image/svg+xml" || file.name.toLowerCase().endsWith(".svg");
  if (isSvg) {
    const reader = new FileReader();
    reader.onload = () => {
      let txt = String(reader.result || "");
      if (!/\bwidth\s*=/i.test(txt)) txt = txt.replace(/<svg\b/i, '<svg width="128" height="128"');
      if (!/\bxmlns\s*=/i.test(txt)) txt = txt.replace(/<svg\b/i, '<svg xmlns="http://www.w3.org/2000/svg"');
      const blob = new Blob([txt], { type: "image/svg+xml;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const img = new Image();
      img.onload = () => { URL.revokeObjectURL(url); applyLoaded(img); };
      img.onerror = () => { URL.revokeObjectURL(url); };
      img.src = url;
    };
    reader.readAsText(file);
    return;
  }
  const url = URL.createObjectURL(file);
  const img = new Image();
  img.onload = () => { URL.revokeObjectURL(url); applyLoaded(img); };
  img.onerror = () => { URL.revokeObjectURL(url); };
  img.src = url;
}

function handleGifUpload(ev) {
  const file = ev.target.files && ev.target.files[0];
  if (file) loadGifSpriteFile(file);
  ev.target.value = "";
}

function computeSafeDrawRect(sw, sh, outW, outH, padBottom) {
  const availW = Math.max(1, outW - 4);
  const availH = Math.max(1, outH - padBottom - 4);
  let dw, dh;
  if (sw <= availW && sh <= availH) {
    const intScale = Math.max(1, Math.floor(Math.min(availW / sw, availH / sh)));
    dw = sw * intScale;
    dh = sh * intScale;
  } else {
    const r = Math.min(availW / sw, availH / sh);
    dw = Math.max(1, Math.floor(sw * r));
    dh = Math.max(1, Math.floor(sh * r));
  }
  const ox = Math.max(0, Math.floor((outW - dw) / 2));
  const oy = Math.max(0, outH - padBottom - dh);
  return { ox, oy, dw, dh };
}

let _gifOffCanvas = null;
let _gifFrameCacheKey = "";
let _gifFrameCache = new Map();

function invalidateGifFrameCache() {
  _gifFrameCacheKey = "";
  _gifFrameCache.clear();
}

function getCachedRiggedFrame(rig, fx, frameIdx, totalFrames) {
  const lockEl = typeof document !== "undefined" ? document.getElementById("gif-outline-lock") : null;
  const lockKey = (!lockEl || lockEl.checked) ? "1" : "0";
  const ampEl = typeof document !== "undefined" ? document.getElementById("gif-amp") : null;
  const ampKey = ampEl ? ampEl.value : "1";
  const techEl = typeof document !== "undefined" ? document.getElementById("gif-tech-select") : null;
  const techKey = techEl ? techEl.value : "bone_fk";
  const key = rig.w + "x" + rig.h + ":" + rig.bodyType + ":" + fx + ":" + totalFrames + ":" + lockKey + ":" + ampKey + ":" + techKey;
  if (_gifFrameCacheKey !== key) {
    _gifFrameCacheKey = key;
    _gifFrameCache.clear();
  }
  let cached = _gifFrameCache.get(frameIdx);
  if (!cached) {
    cached = renderRiggedFrame(rig, fx, frameIdx, totalFrames);
    _gifFrameCache.set(frameIdx, cached);
  }
  return cached;
}

function getGifPlaybackSequence(baseFrames, pingPong) {
  const n = Math.max(1, baseFrames);
  const seq = [];
  for (let i = 0; i < n; i++) seq.push(i);
  if (pingPong && n > 2) {
    for (let i = n - 2; i >= 1; i--) seq.push(i);
  }
  return seq;
}

function getGifBgFill(bgMode, customHex) {
  if (bgMode === "discord") return "#313338";
  if (bgMode === "github") return "#0d1117";
  if (bgMode === "light") return "#ffffff";
  if (bgMode === "custom") return customHex || "#1e1f22";
  return null;
}

function drawNativeRgbaScaled(targetCtx, rgba, sw, sh, outW, outH, padBottom, storeGeom) {
  if (!_gifOffCanvas) _gifOffCanvas = document.createElement("canvas");
  if (_gifOffCanvas.width !== sw || _gifOffCanvas.height !== sh) {
    _gifOffCanvas.width = sw;
    _gifOffCanvas.height = sh;
  }
  _gifOffCanvas.getContext("2d").putImageData(new ImageData(rgba, sw, sh), 0, 0);

  const { ox, oy, dw, dh } = computeSafeDrawRect(sw, sh, outW, outH, padBottom);
  const mirEl = typeof document !== "undefined" ? document.getElementById("gif-mirror") : null;
  const mirrored = Boolean(mirEl && mirEl.checked);
  targetCtx.imageSmoothingEnabled = false;
  if (mirrored) {
    targetCtx.save();
    targetCtx.translate(ox + dw, oy);
    targetCtx.scale(-1, 1);
    targetCtx.drawImage(_gifOffCanvas, 0, 0, sw, sh, 0, 0, dw, dh);
    targetCtx.restore();
  } else {
    targetCtx.drawImage(_gifOffCanvas, 0, 0, sw, sh, ox, oy, dw, dh);
  }
  if (storeGeom) gifLastCanvasGeom = { ox, oy, dw, dh, sw, sh, mirrored };
}

function drawGifMakerFrame(targetCtx, w, h, frameIdx, totalFrames, allowOverlay) {
  const petId = document.getElementById("gif-pet-select").value;
  const fx = document.getElementById("gif-fx-select").value;
  const bgMode = document.getElementById("gif-bg-select").value;
  const customColEl = document.getElementById("gif-bg-color");
  const caption = (document.getElementById("gif-caption").value || "").trim();
  const editPinsEl = document.getElementById("gif-show-rig");
  const mirEl = document.getElementById("gif-mirror");
  const mirrored = Boolean(mirEl && mirEl.checked);
  const editPins = allowOverlay && editPinsEl && editPinsEl.checked;
  targetCtx.clearRect(0, 0, w, h);
  targetCtx.imageSmoothingEnabled = false;

  const amp = w / 128;
  if (bgMode === "card") {
    targetCtx.fillStyle = "#0d1117";
    targetCtx.fillRect(0, 0, w, h);
    targetCtx.strokeStyle = "#3b82f6";
    targetCtx.lineWidth = Math.max(2, Math.round(3 * amp));
    targetCtx.strokeRect(2, 2, w - 4, h - 4);
    if (caption) {
      targetCtx.fillStyle = "#7ee8fa";
      targetCtx.font = "bold " + Math.max(8, Math.round(10 * amp)) + "px monospace";
      targetCtx.fillText(caption, Math.round(8 * amp), Math.round(15 * amp));
    }
  } else {
    const fill = getGifBgFill(bgMode, customColEl ? customColEl.value : "#1e1f22");
    if (fill) {
      targetCtx.fillStyle = fill;
      targetCtx.fillRect(0, 0, w, h);
    }
  }

  const padBottom = bgMode === "card" ? Math.round(8 * amp) : Math.round(2 * amp);
  if (gifCustomSheet && gifCustomSheet.activeIndices.length > 0) {
    const idx = gifCustomSheet.activeIndices[frameIdx % gifCustomSheet.activeIndices.length];
    const tile = gifCustomSheet.frames[idx] || gifCustomSheet.frames[0];
    drawNativeRgbaScaled(targetCtx, tile.rgba, tile.w, tile.h, w, h, padBottom, false);
    return;
  }
  if (gifCustomRig) {
    const frameRgba = editPins ? gifCustomRig.grid : getCachedRiggedFrame(gifCustomRig, fx, frameIdx, totalFrames);
    drawNativeRgbaScaled(targetCtx, frameRgba, gifCustomRig.w, gifCustomRig.h, w, h, padBottom, allowOverlay);
    if (editPins) {
      if (mirrored && gifLastCanvasGeom) {
        const g = gifLastCanvasGeom;
        const mirRig = Object.assign({}, gifCustomRig, {
          pins: gifCustomRig.pins.map((p) => Object.assign({}, p, { x: gifCustomRig.w - 1 - p.x })),
        });
        drawPinOverlay(targetCtx, mirRig, g);
      } else {
        drawPinOverlay(targetCtx, gifCustomRig, gifLastCanvasGeom);
      }
    }
    return;
  }

  if (typeof sprites !== "undefined" && sprites[petId] && sprites[petId].complete && sprites[petId].naturalWidth > 0) {
    const sp = sprites[petId];
    const fw = Math.floor(sp.naturalWidth / 9), fh = sp.naturalHeight;
    const phase = (frameIdx / Math.max(1, totalFrames)) * Math.PI * 2;
    const fCol = (fx === "walk" || fx === "sprint" || fx === "run")
      ? (1 + (frameIdx % 4))
      : (fx === "sleep" ? 6 : 0);
    let dy = 0, dx = 0, scaleY = 1.0;
    if (fx === "hop") dy = -Math.round(Math.sin((frameIdx / totalFrames) * Math.PI) * 14 * amp);
    else if (fx === "sprint") dy = -Math.round(Math.abs(Math.sin(phase)) * 6 * amp);
    else if (fx === "dance") { dx = Math.round(Math.sin(phase) * 5 * amp); dy = -Math.round(Math.abs(Math.sin(phase * 2)) * 5 * amp); }
    else if (fx === "pet") scaleY = 1.0 - Math.max(0, Math.sin(phase)) * 0.14;
    else if (fx === "breathe" || fx === "sleep") scaleY = 1.0 + Math.sin(phase) * 0.04;
    else if (fx === "wave") dx = Math.round(Math.sin(phase) * 2 * amp);

    const box = w - Math.round((bgMode === "card" ? 24 : 8) * amp);
    const drawH = Math.max(1, Math.round(box * scaleY));
    const drawX = Math.floor((w - box) / 2) + dx;
    const drawY = h - padBottom - drawH + dy;
    targetCtx.save();
    if (mirrored) {
      targetCtx.translate(drawX + box, drawY);
      targetCtx.scale(-1, 1);
      targetCtx.drawImage(sp, fCol * fw, 0, fw, fh, 0, 0, box, drawH);
    } else {
      targetCtx.drawImage(sp, fCol * fw, 0, fw, fh, drawX, drawY, box, drawH);
    }
    targetCtx.restore();
  }
}

function onGifBgChange() {
  const bgSel = document.getElementById("gif-bg-select");
  const colEl = document.getElementById("gif-bg-color");
  if (colEl && bgSel) colEl.style.display = bgSel.value === "custom" ? "inline-block" : "none";
  renderGifPreviewNow();
}

function renderGifPreviewNow() {
  const prevC = document.getElementById("gif-preview-canvas");
  if (!prevC) return;
  const fx = document.getElementById("gif-fx-select").value;
  const baseTotal = getGifTotalFrames(fx);
  const ppEl = document.getElementById("gif-pingpong");
  const seq = getGifPlaybackSequence(baseTotal, Boolean(ppEl && ppEl.checked));
  const pos = gifPreviewTick % seq.length;
  const badge = document.getElementById("gif-frame-badge");
  if (badge) badge.textContent = (pos + 1) + "/" + seq.length;
  drawGifMakerFrame(prevC.getContext("2d"), prevC.width, prevC.height, seq[pos], baseTotal, true);
}

function lzwEncodeGif(indices, minCodeSize) {
  const clearCode = 1 << minCodeSize, eoiCode = clearCode + 1;
  let codeSize = minCodeSize + 1, nextCode = eoiCode + 1, dict = new Map();
  const out = [];
  let bitBuf = 0, bitCount = 0;
  function emit(code) {
    bitBuf |= (code << bitCount);
    bitCount += codeSize;
    while (bitCount >= 8) { out.push(bitBuf & 0xff); bitBuf >>= 8; bitCount -= 8; }
  }
  emit(clearCode);
  if (indices.length === 0) { emit(eoiCode); if (bitCount > 0) out.push(bitBuf & 0xff); return out; }
  let prefix = indices[0];
  for (let i = 1; i < indices.length; i++) {
    const k = indices[i], key = (prefix << 8) | k, hit = dict.get(key);
    if (hit !== undefined) { prefix = hit; }
    else {
      emit(prefix);
      if (nextCode < 4096) {
        dict.set(key, nextCode); nextCode++;
        if (nextCode > (1 << codeSize) && codeSize < 12) codeSize++;
      } else { emit(clearCode); dict.clear(); codeSize = minCodeSize + 1; nextCode = eoiCode + 1; }
      prefix = k;
    }
  }
  emit(prefix); emit(eoiCode);
  if (bitCount > 0) out.push(bitBuf & 0xff);
  return out;
}

// Perceptual Oklab color conversion (Björn Ottosson, 2020)
function rgbToOklab(r, g, b) {
  const sr = r / 255, sg = g / 255, sb = b / 255;
  const lr = sr <= 0.04045 ? sr / 12.92 : Math.pow((sr + 0.055) / 1.055, 2.4);
  const lg = sg <= 0.04045 ? sg / 12.92 : Math.pow((sg + 0.055) / 1.055, 2.4);
  const lb = sb <= 0.04045 ? sb / 12.92 : Math.pow((sb + 0.055) / 1.055, 2.4);
  const l = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb);
  const m = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb);
  const s = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb);
  return [
    0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
  ];
}

const BAYER_4X4 = [
  0, 8, 2, 10,
  12, 4, 14, 6,
  3, 11, 1, 9,
  15, 7, 13, 5,
];

function buildGifPalette(framesRgba, transparent) {
  const uniqueMap = new Map(), counts = [];
  for (let f = 0; f < framesRgba.length; f++) {
    const rgba = framesRgba[f];
    for (let p = 0; p < rgba.length; p += 4) {
      if (transparent && rgba[p + 3] < 128) continue;
      const r = rgba[p], g = rgba[p + 1], b = rgba[p + 2], key = (r << 16) | (g << 8) | b;
      const idx = uniqueMap.get(key);
      if (idx === undefined) {
        const lab = rgbToOklab(r, g, b);
        uniqueMap.set(key, counts.length);
        counts.push([r, g, b, 1, lab[0], lab[1], lab[2]]);
      } else {
        counts[idx][3]++;
      }
    }
  }
  const palette = [[0, 0, 0]];
  if (counts.length <= 255) {
    for (let i = 0; i < counts.length; i++) palette.push([counts[i][0], counts[i][1], counts[i][2]]);
    return palette;
  }
  // Perceptual Oklab Median-Cut Quantization
  const boxes = [counts];
  while (boxes.length < 255) {
    let splitIdx = -1, bestRange = -1, splitChan = 4;
    for (let i = 0; i < boxes.length; i++) {
      const box = boxes[i];
      if (box.length < 2) continue;
      let minL = 2, maxL = -2, minA = 2, maxA = -2, minB = 2, maxB = -2;
      for (let j = 0; j < box.length; j++) {
        const c = box[j];
        if (c[4] < minL) minL = c[4]; if (c[4] > maxL) maxL = c[4];
        if (c[5] < minA) minA = c[5]; if (c[5] > maxA) maxA = c[5];
        if (c[6] < minB) minB = c[6]; if (c[6] > maxB) maxB = c[6];
      }
      const rL = (maxL - minL) * 1.15, rA = maxA - minA, rB = maxB - minB;
      const m = Math.max(rL, rA, rB);
      if (m > bestRange) { bestRange = m; splitIdx = i; splitChan = m === rL ? 4 : (m === rA ? 5 : 6); }
    }
    if (splitIdx === -1 || bestRange <= 1e-5) break;
    const box = boxes.splice(splitIdx, 1)[0];
    box.sort((a, b) => a[splitChan] - b[splitChan]);
    const mid = box.length >> 1;
    boxes.push(box.slice(0, mid), box.slice(mid));
  }
  for (let i = 0; i < boxes.length; i++) {
    const box = boxes[i];
    let sr = 0, sg = 0, sb = 0, sw = 0;
    for (let j = 0; j < box.length; j++) { const wt = box[j][3]; sr += box[j][0] * wt; sg += box[j][1] * wt; sb += box[j][2] * wt; sw += wt; }
    if (sw > 0) palette.push([Math.round(sr / sw), Math.round(sg / sw), Math.round(sb / sw)]);
  }
  return palette;
}

function encodeGif89a(framesRgba, w, h, delayCs, transparent, opts) {
  const useDither = Boolean(opts && opts.dither);
  const useDirtyRect = !(opts && opts.optimize === false);
  const palette = buildGifPalette(framesRgba, transparent);
  const palLab = palette.map((c) => rgbToOklab(c[0], c[1], c[2]));
  const lookup = new Map();
  for (let i = 1; i < palette.length; i++) {
    const c = palette[i];
    lookup.set((c[0] << 16) | (c[1] << 8) | c[2], i);
  }
  const nearestColor = (r, g, b) => {
    const key = (r << 16) | (g << 8) | b;
    let ci = lookup.get(key);
    if (ci !== undefined) return ci;
    const lab = rgbToOklab(r, g, b);
    let best = 1, bestD = 1e9;
    for (let j = 1; j < palLab.length; j++) {
      const dL = lab[0] - palLab[j][0], dA = lab[1] - palLab[j][1], dB = lab[2] - palLab[j][2];
      const d = dL * dL + dA * dA + dB * dB;
      if (d < bestD) { bestD = d; best = j; }
    }
    lookup.set(key, best);
    return best;
  };

  const indexedFrames = [];
  for (let f = 0; f < framesRgba.length; f++) {
    const rgba = framesRgba[f], idxBuf = new Uint8Array(w * h);
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const p = y * w + x;
        if (transparent && rgba[p * 4 + 3] < 128) { idxBuf[p] = 0; continue; }
        let r = rgba[p * 4], g = rgba[p * 4 + 1], b = rgba[p * 4 + 2];
        if (useDither && palette.length > 16) {
          const thr = (BAYER_4X4[(y & 3) * 4 + (x & 3)] / 16 - 0.5) * 14;
          r = Math.max(0, Math.min(255, Math.round(r + thr)));
          g = Math.max(0, Math.min(255, Math.round(g + thr)));
          b = Math.max(0, Math.min(255, Math.round(b + thr)));
        }
        idxBuf[p] = nearestColor(r, g, b);
      }
    }
    indexedFrames.push(idxBuf);
  }

  const bytes = [];
  const pushStr = (s) => { for (let i = 0; i < s.length; i++) bytes.push(s.charCodeAt(i)); };
  const pushU16 = (v) => { bytes.push(v & 0xff, (v >> 8) & 0xff); };
  pushStr("GIF89a"); pushU16(w); pushU16(h);
  bytes.push(0xF7, 0x00, 0x00);
  for (let i = 0; i < 256; i++) { const c = palette[i] || [0, 0, 0]; bytes.push(c[0], c[1], c[2]); }
  bytes.push(0x21, 0xFF, 0x0B); pushStr("NETSCAPE2.0"); bytes.push(0x03, 0x01, 0x00, 0x00, 0x00);

  for (let f = 0; f < indexedFrames.length; f++) {
    const cur = indexedFrames[f];
    let rx = 0, ry = 0, rw = w, rh = h;
    let frameData = cur;
    let gceFlags = transparent ? 0x09 : 0x04;

    if (useDirtyRect && !transparent && f > 0) {
      // Opaque-BG Dirty-Rect + Transparency Delta Optimization (disposal=Do Not Dispose, 0x05)
      const prev = indexedFrames[f - 1];
      let minX = w, minY = h, maxX = -1, maxY = -1;
      for (let y = 0; y < h; y++) {
        for (let x = 0; x < w; x++) {
          const p = y * w + x;
          if (cur[p] !== prev[p]) {
            if (x < minX) minX = x; if (x > maxX) maxX = x;
            if (y < minY) minY = y; if (y > maxY) maxY = y;
          }
        }
      }
      if (maxX >= minX) {
        rx = minX; ry = minY; rw = maxX - minX + 1; rh = maxY - minY + 1;
        frameData = new Uint8Array(rw * rh);
        for (let y = 0; y < rh; y++) {
          for (let x = 0; x < rw; x++) {
            const sp = (ry + y) * w + (rx + x);
            frameData[y * rw + x] = cur[sp] === prev[sp] ? 0 : cur[sp];
          }
        }
        gceFlags = 0x05; // disposal=1 (keep), transparent=1 (index 0 skips unchanged pixels)
      } else {
        rx = 0; ry = 0; rw = 1; rh = 1;
        frameData = new Uint8Array([0]);
        gceFlags = 0x05;
      }
    } else if (useDirtyRect && transparent) {
      // Transparent-BG tight bounding-box crop (disposal=2 restore to background, 0x09)
      let minX = w, minY = h, maxX = -1, maxY = -1;
      for (let y = 0; y < h; y++) {
        for (let x = 0; x < w; x++) {
          if (cur[y * w + x] !== 0) {
            if (x < minX) minX = x; if (x > maxX) maxX = x;
            if (y < minY) minY = y; if (y > maxY) maxY = y;
          }
        }
      }
      if (maxX >= minX) {
        rx = minX; ry = minY; rw = maxX - minX + 1; rh = maxY - minY + 1;
        frameData = new Uint8Array(rw * rh);
        for (let y = 0; y < rh; y++) {
          frameData.set(cur.subarray((ry + y) * w + rx, (ry + y) * w + rx + rw), y * rw);
        }
      }
    }

    bytes.push(0x21, 0xF9, 0x04, gceFlags);
    pushU16(delayCs); bytes.push(0x00, 0x00, 0x2C);
    pushU16(rx); pushU16(ry); pushU16(rw); pushU16(rh); bytes.push(0x00, 8);
    const lzw = lzwEncodeGif(frameData, 8);
    for (let i = 0; i < lzw.length; i += 255) {
      const len = Math.min(255, lzw.length - i);
      bytes.push(len);
      for (let j = 0; j < len; j++) bytes.push(lzw[i + j]);
    }
    bytes.push(0x00);
  }
  bytes.push(0x3B);
  return new Uint8Array(bytes);
}

// Pure-JS 24-bit APNG Assembler (full 24-bit RGB + smooth 8-bit Alpha)
const _CRC32_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = (c & 1) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1);
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32Bytes(buf) {
  let c = 0xFFFFFFFF;
  for (let i = 0; i < buf.length; i++) c = _CRC32_TABLE[(c ^ buf[i]) & 0xFF] ^ (c >>> 8);
  return (c ^ 0xFFFFFFFF) >>> 0;
}

function makePngChunk(typeStr, data) {
  const out = new Uint8Array(12 + data.length);
  const dv = new DataView(out.buffer);
  dv.setUint32(0, data.length, false);
  for (let i = 0; i < 4; i++) out[4 + i] = typeStr.charCodeAt(i);
  out.set(data, 8);
  dv.setUint32(8 + data.length, crc32Bytes(out.subarray(4, 8 + data.length)), false);
  return out;
}

function parsePngChunks(pngBytes) {
  const chunks = [];
  const dv = new DataView(pngBytes.buffer, pngBytes.byteOffset, pngBytes.byteLength);
  let pos = 8;
  while (pos + 12 <= pngBytes.length) {
    const len = dv.getUint32(pos, false);
    const type = String.fromCharCode(pngBytes[pos + 4], pngBytes[pos + 5], pngBytes[pos + 6], pngBytes[pos + 7]);
    const data = pngBytes.subarray(pos + 8, pos + 8 + len);
    chunks.push({ type, data });
    pos += 12 + len;
    if (type === "IEND") break;
  }
  return chunks;
}

function assembleApngFromPngFrames(pngBuffers, delayMs) {
  if (!pngBuffers || pngBuffers.length === 0) return new Uint8Array(0);
  const firstChunks = parsePngChunks(pngBuffers[0]);
  const ihdr = firstChunks.find((c) => c.type === "IHDR");
  if (!ihdr) throw new Error("Missing IHDR in PNG frame");
  const ihdrView = new DataView(ihdr.data.buffer, ihdr.data.byteOffset, ihdr.data.byteLength);
  const w = ihdrView.getUint32(0, false);
  const h = ihdrView.getUint32(4, false);

  const parts = [new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10])];
  parts.push(makePngChunk("IHDR", ihdr.data));

  // acTL: num_frames, num_plays = 0 (infinite loop)
  const actl = new Uint8Array(8);
  const actlDv = new DataView(actl.buffer);
  actlDv.setUint32(0, pngBuffers.length, false);
  actlDv.setUint32(4, 0, false);
  parts.push(makePngChunk("acTL", actl));

  let seq = 0;
  const delayNum = Math.max(10, Math.round(delayMs));
  const delayDen = 1000;

  for (let f = 0; f < pngBuffers.length; f++) {
    const chunks = f === 0 ? firstChunks : parsePngChunks(pngBuffers[f]);
    const idats = chunks.filter((c) => c.type === "IDAT");

    // fcTL (26 bytes)
    const fctl = new Uint8Array(26);
    const fdv = new DataView(fctl.buffer);
    fdv.setUint32(0, seq++, false);
    fdv.setUint32(4, w, false);
    fdv.setUint32(8, h, false);
    fdv.setUint32(12, 0, false);
    fdv.setUint32(16, 0, false);
    fdv.setUint16(20, delayNum, false);
    fdv.setUint16(22, delayDen, false);
    fctl[24] = 1; // APNG_DISPOSE_OP_BACKGROUND
    fctl[25] = 0; // APNG_BLEND_OP_SOURCE
    parts.push(makePngChunk("fcTL", fctl));

    if (f === 0) {
      for (const idat of idats) parts.push(makePngChunk("IDAT", idat.data));
    } else {
      for (const idat of idats) {
        const fdat = new Uint8Array(4 + idat.data.length);
        new DataView(fdat.buffer).setUint32(0, seq++, false);
        fdat.set(idat.data, 4);
        parts.push(makePngChunk("fdAT", fdat));
      }
    }
  }
  parts.push(makePngChunk("IEND", new Uint8Array(0)));

  let totalLen = 0;
  for (const p of parts) totalLen += p.length;
  const out = new Uint8Array(totalLen);
  let off = 0;
  for (const p of parts) { out.set(p, off); off += p.length; }
  return out;
}

function buildDeskpetXmlString(title, tilesX, pngBase64) {
  const safeTitle = String(title || "Custom Pet").replace(/[<>&"]/g, "");
  const walkFrames = [];
  for (let i = 0; i < tilesX; i++) walkFrames.push("      <Frame>" + i + "</Frame>");
  return [
    '<?xml version="1.0" encoding="utf-8"?>',
    '<Animations>',
    '  <Header>',
    '    <Title>' + safeTitle + '</Title>',
    '    <Author>diypets Web GIF Maker</Author>',
    '    <Version>1.0</Version>',
    '  </Header>',
    '  <Image>',
    '    <TilesX>' + tilesX + '</TilesX>',
    '    <TilesY>1</TilesY>',
    '    <Png>' + pngBase64 + '</Png>',
    '  </Image>',
    '  <Animation id="1" name="walk">',
    '    <Sequence repeat="0">',
    walkFrames.join('\n'),
    '    </Sequence>',
    '  </Animation>',
    '  <Animation id="2" name="idle">',
    '    <Sequence repeat="0">',
    '      <Frame>0</Frame>',
    '    </Sequence>',
    '  </Animation>',
    '</Animations>',
  ].join('\n');
}

function getGifTotalFrames(fx) {
  if (gifCustomSheet && gifCustomSheet.activeIndices.length > 0) return gifCustomSheet.activeIndices.length;
  if (fx === "breathe") return 4;
  if (fx === "run") return 6;
  if (fx === "walk" && !gifCustomRig) return 4;
  return 8;
}

function exportMascotGif() {
  const statusEl = document.getElementById("gif-status");
  try {
    const size = Math.max(64, Math.min(256, Number(document.getElementById("gif-size-select").value) || 128));
    const off = document.createElement("canvas");
    off.width = size; off.height = size;
    const octx = off.getContext("2d", { willReadFrequently: true });
    const fx = document.getElementById("gif-fx-select").value;
    const petId = document.getElementById("gif-pet-select").value;
    const transparent = document.getElementById("gif-bg-select").value === "transparent";
    const fps = Math.max(4, Math.min(14, Number(document.getElementById("gif-fps").value) || 8));
    const ppEl = document.getElementById("gif-pingpong");

    if (!gifCustomRig && !gifCustomSheet && (!sprites[petId] || !sprites[petId].complete || sprites[petId].naturalWidth === 0)) {
      statusEl.textContent = "WAIT: SPRITE STILL LOADING...";
      return;
    }
    const baseTotal = getGifTotalFrames(fx);
    const seq = getGifPlaybackSequence(baseTotal, Boolean(ppEl && ppEl.checked));
    const frames = [];
    for (let i = 0; i < seq.length; i++) {
      drawGifMakerFrame(octx, size, size, seq[i], baseTotal, false);
      frames.push(octx.getImageData(0, 0, size, size).data);
    }
    const ditherEl = document.getElementById("gif-dither");
    const gifBytes = encodeGif89a(frames, size, size, Math.max(4, Math.round(100 / fps)), transparent, { dither: Boolean(ditherEl && ditherEl.checked), optimize: true });
    const blob = new Blob([gifBytes], { type: "image/gif" });
    const slug = ((gifCustomRig || gifCustomSheet) && gifCustomName) ? gifCustomName : petId;
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = slug + "-" + (gifCustomSheet ? "sheet" : fx) + "-" + size + "px.gif";
    document.body.appendChild(a); a.click(); a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 2500);
    statusEl.textContent = "EXPORTED " + (gifBytes.length / 1024).toFixed(1) + " KB GIF (" + size + "px, " + seq.length + "f)";
    if (typeof showToast === "function") showToast("Exported " + a.download);
  } catch (err) {
    console.error("GIF export error:", err);
    statusEl.textContent = "ERROR: USE HTTPS OR UPLOAD A LOCAL FILE";
  }
}

function exportMascotSpriteSheet() {
  const statusEl = document.getElementById("gif-status");
  try {
    const size = Math.max(64, Math.min(256, Number(document.getElementById("gif-size-select").value) || 128));
    const fx = document.getElementById("gif-fx-select").value;
    const petId = document.getElementById("gif-pet-select").value;
    const ppEl = document.getElementById("gif-pingpong");
    const baseTotal = getGifTotalFrames(fx);
    const seq = getGifPlaybackSequence(baseTotal, Boolean(ppEl && ppEl.checked));

    const tileC = document.createElement("canvas");
    tileC.width = size; tileC.height = size;
    const tctx = tileC.getContext("2d");

    const sheetC = document.createElement("canvas");
    sheetC.width = size * seq.length; sheetC.height = size;
    const sctx = sheetC.getContext("2d");
    sctx.imageSmoothingEnabled = false;

    for (let i = 0; i < seq.length; i++) {
      drawGifMakerFrame(tctx, size, size, seq[i], baseTotal, false);
      sctx.drawImage(tileC, i * size, 0);
    }

    sheetC.toBlob((blob) => {
      if (!blob) return;
      const slug = ((gifCustomRig || gifCustomSheet) && gifCustomName) ? gifCustomName : petId;
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = slug + "-" + (gifCustomSheet ? "sheet" : fx) + "-" + seq.length + "x1-" + size + "px.png";
      document.body.appendChild(a); a.click(); a.remove();
      setTimeout(() => URL.revokeObjectURL(a.href), 2500);
      statusEl.textContent = "EXPORTED " + seq.length + "x1 PNG SHEET (" + (blob.size / 1024).toFixed(1) + " KB)";
      if (typeof showToast === "function") showToast("Exported PNG sprite sheet (" + seq.length + " frames)");
    }, "image/png");
  } catch (err) {
    console.error("Sheet export error:", err);
    statusEl.textContent = "ERROR EXPORTING PNG SHEET";
  }
}


function exportMascotAllFxSheet() {
  const statusEl = document.getElementById("gif-status");
  try {
    const size = Math.max(64, Math.min(256, Number(document.getElementById("gif-size-select").value) || 128));
    const petId = document.getElementById("gif-pet-select").value;
    const fxSel = document.getElementById("gif-fx-select");
    const prevFx = fxSel ? fxSel.value : "walk";
    const fxRows = [
      { id: "walk", label: "WALK" },
      { id: "sprint", label: "SPRINT" },
      { id: "dance", label: "DANCE" },
      { id: "hop", label: "HOP" },
      { id: "sleep", label: "SLEEP" },
    ];
    const cols = 8;
    const labelW = 64;

    const tileC = document.createElement("canvas");
    tileC.width = size; tileC.height = size;
    const tctx = tileC.getContext("2d");

    const sheetC = document.createElement("canvas");
    sheetC.width = labelW + size * cols;
    sheetC.height = size * fxRows.length;
    const sctx = sheetC.getContext("2d");
    sctx.imageSmoothingEnabled = false;

    for (let r = 0; r < fxRows.length; r++) {
      const rowFx = fxRows[r];
      if (fxSel) fxSel.value = rowFx.id;
      const ry = r * size;
      sctx.fillStyle = "rgba(13, 17, 23, 0.85)";
      sctx.fillRect(0, ry, labelW - 2, size - 2);
      sctx.fillStyle = "#7ee8fa";
      sctx.font = "bold 11px monospace";
      sctx.fillText(rowFx.label, 8, ry + Math.floor(size / 2) + 4);
      for (let c = 0; c < cols; c++) {
        drawGifMakerFrame(tctx, size, size, c, cols, false);
        sctx.drawImage(tileC, labelW + c * size, ry);
      }
    }
    if (fxSel) fxSel.value = prevFx;

    sheetC.toBlob((blob) => {
      if (!blob) return;
      const slug = ((gifCustomRig || gifCustomSheet) && gifCustomName) ? gifCustomName : petId;
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = slug + "-all-fx-" + cols + "x" + fxRows.length + "-" + size + "px.png";
      document.body.appendChild(a); a.click(); a.remove();
      setTimeout(() => URL.revokeObjectURL(a.href), 2500);
      statusEl.textContent = "EXPORTED ALL-FX " + cols + "x" + fxRows.length + " SHEET (" + (blob.size / 1024).toFixed(1) + " KB)";
      if (typeof showToast === "function") showToast("Exported full multi-FX PNG sheet (" + fxRows.length + " rows x " + cols + " frames)");
    }, "image/png");
  } catch (err) {
    console.error("All-FX sheet export error:", err);
    statusEl.textContent = "ERROR EXPORTING ALL-FX SHEET";
  }
}



// Count enclosed transparent interior holes/slits (border flood-fill of alpha===0; anything left is a tear/hole)
function countInteriorHoles(rgba, w, h) {
  const n = w * h;
  const vis = new Uint8Array(n);
  const q = new Int32Array(n);
  let head = 0, tail = 0;
  const push = (idx) => {
    if (!vis[idx] && rgba[idx * 4 + 3] === 0) { vis[idx] = 1; q[tail++] = idx; }
  };
  for (let x = 0; x < w; x++) { push(x); push((h - 1) * w + x); }
  for (let y = 0; y < h; y++) { push(y * w); push(y * w + (w - 1)); }
  while (head < tail) {
    const u = q[head++], ux = u % w, uy = (u / w) | 0;
    if (ux > 0) push(u - 1);
    if (ux + 1 < w) push(u + 1);
    if (uy > 0) push(u - w);
    if (uy + 1 < h) push(u + w);
  }
  let holes = 0;
  const holeMask = new Uint8Array(n);
  for (let i = 0; i < n; i++) {
    if (!vis[i] && rgba[i * 4 + 3] === 0) {
      holeMask[i] = 1;
      holes++;
    }
  }
  return { holes, holeMask };
}

// Build a true visual diff image comparing `cur` against `rest`:
// - Darkened silhouette = unchanged pixels
// - Bright Green = newly occupied pixels (real motion)
// - Cyan/Blue = vacated rest pixels
// - Bright Red = newly opened interior holes/slits (tears!)
function buildFrameDiffRgba(restRgba, curRgba, w, h) {
  const out = new Uint8ClampedArray(w * h * 4);
  const restH = countInteriorHoles(restRgba, w, h);
  const curH = countInteriorHoles(curRgba, w, h);
  let movedPx = 0, newHolePx = 0;
  for (let i = 0; i < w * h; i++) {
    const p = i * 4;
    const rA = restRgba[p + 3] > 0;
    const cA = curRgba[p + 3] > 0;
    if (curH.holeMask[i] && !restH.holeMask[i]) {
      // Newly opened interior tear/slit -> BRIGHT RED
      out[p] = 255; out[p + 1] = 35; out[p + 2] = 60; out[p + 3] = 255;
      newHolePx++;
    } else if (cA && !rA) {
      // Newly occupied motion pixel -> BRIGHT GREEN
      out[p] = 50; out[p + 1] = 235; out[p + 2] = 120; out[p + 3] = 255;
      movedPx++;
    } else if (rA && !cA) {
      // Vacated rest pixel -> BLUE
      out[p] = 65; out[p + 1] = 145; out[p + 2] = 255; out[p + 3] = 200;
      movedPx++;
    } else if (cA) {
      // Unchanged interior -> subtle muted greyscale so motion & tears pop
      const lum = Math.round((curRgba[p] * 0.3 + curRgba[p + 1] * 0.59 + curRgba[p + 2] * 0.11) * 0.45);
      out[p] = lum; out[p + 1] = lum; out[p + 2] = lum; out[p + 3] = 255;
    }
  }
  return { rgba: out, movedPx, newHolePx };
}

function exportMascotCompareSheet() {
  const statusEl = document.getElementById("gif-status");
  try {
    const size = Math.max(64, Math.min(256, Number(document.getElementById("gif-size-select").value) || 128));
    const petId = document.getElementById("gif-pet-select").value;
    const fx = document.getElementById("gif-fx-select").value;
    const techSel = document.getElementById("gif-tech-select");
    const bgSel = document.getElementById("gif-bg-select");
    const prevTech = techSel ? techSel.value : "rigid_3pass";
    const prevBg = bgSel ? bgSel.value : "transparent";
    const cols = getGifTotalFrames(fx);
    const labelW = 92;

    const tileC = document.createElement("canvas");
    tileC.width = size; tileC.height = size;
    const tctx = tileC.getContext("2d", { willReadFrequently: true });

    // Capture Rest reference frame (unwarped grid or frame 0)
    if (bgSel) bgSel.value = "transparent";
    tctx.clearRect(0, 0, size, size);
    if (gifCustomRig) {
      drawNativeRgbaScaled(tctx, gifCustomRig.grid, gifCustomRig.w, gifCustomRig.h, size, size, Math.round(2 * (size / 128)), false);
    } else {
      drawGifMakerFrame(tctx, size, size, 0, cols, false);
    }
    const restRgba = tctx.getImageData(0, 0, size, size).data;

    const techs = [
      { id: "bone_fk", label: "BONE FK", color: "#7ee8fa" },
      { id: "harmonic_dqs", label: "HARMONIC", color: "#a3e635" },
      { id: "fabrik_ik", label: "FABRIK IK", color: "#f472b6" },
      { id: "layered_inpaint", label: "INPAINT", color: "#38bdf8" },
      { id: "complementary_dyn", label: "COMP DYN", color: "#ffd166" },
      { id: "arap_2d", label: "ARAP 2D", color: "#fb923c" },
      { id: "cleanedge_rot", label: "CLEANEDGE", color: "#c084fc" },
      { id: "parallax_25d", label: "2.5D PAR", color: "#2dd4bf" },
      { id: "sota_hybrid", label: "SOTA HYB", color: "#f43f5e" },
    ];

    const sheetC = document.createElement("canvas");
    sheetC.width = labelW + size * cols;
    sheetC.height = size * (techs.length * 2);
    const sctx = sheetC.getContext("2d");
    sctx.imageSmoothingEnabled = false;

    const diffC = document.createElement("canvas");
    diffC.width = size; diffC.height = size;
    const dctx = diffC.getContext("2d");

    for (let ti = 0; ti < techs.length; ti++) {
      const tech = techs[ti];
      if (techSel) techSel.value = tech.id;
      invalidateGifFrameCache();

      const rowRenderY = (ti * 2) * size;
      const rowDiffY = (ti * 2 + 1) * size;
      let totalMoved = 0, totalNewHoles = 0;

      for (let c = 0; c < cols; c++) {
        drawGifMakerFrame(tctx, size, size, c, cols, false);
        sctx.drawImage(tileC, labelW + c * size, rowRenderY);

        const curRgba = tctx.getImageData(0, 0, size, size).data;
        const diff = buildFrameDiffRgba(restRgba, curRgba, size, size);
        totalMoved += diff.movedPx;
        totalNewHoles += diff.newHolePx;

        // Dark background on diff row so green/blue/red motion & tear markers are crystal clear
        sctx.fillStyle = "#11161f";
        sctx.fillRect(labelW + c * size, rowDiffY, size, size);
        dctx.putImageData(new ImageData(diff.rgba, size, size), 0, 0);
        sctx.drawImage(diffC, labelW + c * size, rowDiffY);
      }

      // Render row label
      sctx.fillStyle = "#0d1117";
      sctx.fillRect(0, rowRenderY, labelW - 2, size - 2);
      sctx.fillStyle = tech.color;
      sctx.font = "bold 11px monospace";
      sctx.fillText(tech.label, 6, rowRenderY + Math.floor(size / 2) + 4);

      // Diff row label with exact numeric motion & hole metrics
      sctx.fillStyle = "#161b22";
      sctx.fillRect(0, rowDiffY, labelW - 2, size - 2);
      sctx.fillStyle = "#a3e635";
      sctx.font = "bold 10px monospace";
      sctx.fillText("DIFF " + Math.round(totalMoved / cols) + "px", 6, rowDiffY + Math.floor(size / 2) - 4);
      sctx.fillStyle = totalNewHoles > 0 ? "#ff4d6d" : "#7ee8fa";
      sctx.fillText("HOLES: " + totalNewHoles, 6, rowDiffY + Math.floor(size / 2) + 12);
    }

    if (techSel) techSel.value = prevTech;
    if (bgSel) bgSel.value = prevBg;
    invalidateGifFrameCache();

    sheetC.toBlob((blob) => {
      if (!blob) return;
      const slug = ((gifCustomRig || gifCustomSheet) && gifCustomName) ? gifCustomName : petId;
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = slug + "-compare-diff-" + fx + "-" + cols + "x4-" + size + "px.png";
      document.body.appendChild(a); a.click(); a.remove();
      setTimeout(() => URL.revokeObjectURL(a.href), 2500);
      statusEl.textContent = "EXPORTED WHOLE-DIFF COMPARISON (" + fx.toUpperCase() + ", " + cols + "x4)";
      if (typeof showToast === "function") showToast("Exported 4-row Whole-Diff comparison (Green=motion, Red=tears)");
    }, "image/png");
  } catch (err) {
    console.error("Compare diff sheet export error:", err);
    statusEl.textContent = "ERROR EXPORTING COMPARE DIFF";
  }
}

function exportMascotXmlPack() {
  const statusEl = document.getElementById("gif-status");
  try {
    const size = Math.max(64, Math.min(256, Number(document.getElementById("gif-size-select").value) || 128));
    const fx = document.getElementById("gif-fx-select").value;
    const petId = document.getElementById("gif-pet-select").value;
    const title = (document.getElementById("gif-caption").value || petId).trim();
    const baseTotal = getGifTotalFrames(fx);

    const tileC = document.createElement("canvas");
    tileC.width = size; tileC.height = size;
    const tctx = tileC.getContext("2d");

    const sheetC = document.createElement("canvas");
    sheetC.width = size * baseTotal; sheetC.height = size;
    const sctx = sheetC.getContext("2d");
    sctx.imageSmoothingEnabled = false;

    const bgEl = document.getElementById("gif-bg-select");
    const prevBg = bgEl ? bgEl.value : "transparent";
    if (bgEl) bgEl.value = "transparent";
    for (let i = 0; i < baseTotal; i++) {
      drawGifMakerFrame(tctx, size, size, i, baseTotal, false);
      sctx.drawImage(tileC, i * size, 0);
    }
    if (bgEl) bgEl.value = prevBg;

    const b64 = sheetC.toDataURL("image/png").replace(/^data:image\/png;base64,/, "");
    const xml = buildDeskpetXmlString(title, baseTotal, b64);
    const blob = new Blob([xml], { type: "application/xml" });
    const slug = ((gifCustomRig || gifCustomSheet) && gifCustomName) ? gifCustomName : petId;
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = slug + "-deskpet.xml";
    document.body.appendChild(a); a.click(); a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 2500);
    statusEl.textContent = "EXPORTED DESKPET.XML (" + (blob.size / 1024).toFixed(1) + " KB, " + baseTotal + "f)";
    if (typeof showToast === "function") showToast("Exported " + a.download + " (drop into diypets!)");
  } catch (err) {
    console.error("XML pack export error:", err);
    statusEl.textContent = "ERROR EXPORTING DESKPET.XML";
  }
}

let gifChromaKeyRgb = null;
let gifPickingKey = false;

function toggleGifKeyPicker() {
  if (!gifCustomRawImg) {
    if (typeof showToast === "function") showToast("Upload a sprite first to pick a chroma-key background color");
    return;
  }
  gifPickingKey = !gifPickingKey;
  const btn = document.getElementById("gif-pick-key-btn");
  const prevC = document.getElementById("gif-preview-canvas");
  if (btn) btn.style.borderColor = gifPickingKey ? "#7ee8fa" : "";
  if (prevC) prevC.style.cursor = gifPickingKey ? "crosshair" : "";
}

function clearGifChromaKey() {
  gifChromaKeyRgb = null;
  gifPickingKey = false;
  const sw = document.getElementById("gif-key-swatch");
  const btn = document.getElementById("gif-pick-key-btn");
  const prevC = document.getElementById("gif-preview-canvas");
  if (sw) { sw.style.background = "transparent"; sw.title = "Auto corner background"; }
  if (btn) btn.style.borderColor = "";
  if (prevC) prevC.style.cursor = "";
  refreshCleanedCustomImg();
}

function onGifAmpChange() {
  const el = document.getElementById("gif-amp");
  const lbl = document.getElementById("gif-amp-val");
  if (el && lbl) lbl.textContent = Number(el.value).toFixed(1) + "x";
  invalidateGifFrameCache();
  renderGifPreviewNow();
}

function onGifCleanParamChange() {
  const tolEl = document.getElementById("gif-key-tol");
  const tolLbl = document.getElementById("gif-key-tol-val");
  if (tolEl && tolLbl) tolLbl.textContent = String(tolEl.value);
  const defEl = document.getElementById("gif-defringe");
  const defLbl = document.getElementById("gif-defringe-val");
  if (defEl && defLbl) defLbl.textContent = String(defEl.value);
  refreshCleanedCustomImg();
}

async function exportMascotApng() {
  const statusEl = document.getElementById("gif-status");
  try {
    const size = Math.max(64, Math.min(256, Number(document.getElementById("gif-size-select").value) || 128));
    const off = document.createElement("canvas");
    off.width = size; off.height = size;
    const octx = off.getContext("2d", { willReadFrequently: true });
    const fx = document.getElementById("gif-fx-select").value;
    const petId = document.getElementById("gif-pet-select").value;
    const fps = Math.max(4, Math.min(14, Number(document.getElementById("gif-fps").value) || 8));
    const ppEl = document.getElementById("gif-pingpong");
    const baseTotal = getGifTotalFrames(fx);
    const seq = getGifPlaybackSequence(baseTotal, Boolean(ppEl && ppEl.checked));

    const pngFrames = [];
    for (let i = 0; i < seq.length; i++) {
      drawGifMakerFrame(octx, size, size, seq[i], baseTotal, false);
      const blob = await new Promise((res) => off.toBlob(res, "image/png"));
      if (!blob) throw new Error("Canvas toBlob failed");
      pngFrames.push(new Uint8Array(await blob.arrayBuffer()));
    }
    const apngBytes = assembleApngFromPngFrames(pngFrames, Math.round(1000 / fps));
    const blob = new Blob([apngBytes], { type: "image/apng" });
    const slug = ((gifCustomRig || gifCustomSheet) && gifCustomName) ? gifCustomName : petId;
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = slug + "-" + (gifCustomSheet ? "sheet" : fx) + "-" + size + "px-apng.png";
    document.body.appendChild(a); a.click(); a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 2500);
    statusEl.textContent = "EXPORTED " + (apngBytes.length / 1024).toFixed(1) + " KB 24-BIT APNG (" + size + "px, " + seq.length + "f)";
    if (typeof showToast === "function") showToast("Exported 24-bit APNG: " + a.download);
  } catch (err) {
    console.error("APNG export error:", err);
    statusEl.textContent = "ERROR EXPORTING APNG";
  }
}

function updateGifPreviewFps() {
  const fpsEl = document.getElementById("gif-fps");
  if (!fpsEl) return;
  const fps = Math.max(4, Math.min(14, Number(fpsEl.value) || 8));
  const lbl = document.getElementById("gif-fps-val");
  if (lbl) lbl.textContent = String(fps);
  if (gifPreviewTimer) clearInterval(gifPreviewTimer);
  gifPreviewTimer = setInterval(() => {
    const prevC = document.getElementById("gif-preview-canvas");
    if (!prevC) return;
    const fx = document.getElementById("gif-fx-select").value;
    const baseTotal = getGifTotalFrames(fx);
    const ppEl = document.getElementById("gif-pingpong");
    const seq = getGifPlaybackSequence(baseTotal, Boolean(ppEl && ppEl.checked));
    if (!gifPaused) gifPreviewTick = (gifPreviewTick + 1) % seq.length;
    const pos = gifPreviewTick % seq.length;
    const badge = document.getElementById("gif-frame-badge");
    if (badge) badge.textContent = (pos + 1) + "/" + seq.length;
    drawGifMakerFrame(prevC.getContext("2d"), prevC.width, prevC.height, seq[pos], baseTotal, true);
  }, Math.round(1000 / fps));
}

if (typeof document !== "undefined") {
  const dz = document.getElementById("gif-drop-zone");
  if (dz) {
    dz.addEventListener("dragover", (e) => { e.preventDefault(); dz.style.borderColor = "#7ee8fa"; });
    dz.addEventListener("dragleave", () => { dz.style.borderColor = ""; });
    dz.addEventListener("drop", (e) => {
      e.preventDefault(); dz.style.borderColor = "";
      const file = e.dataTransfer && e.dataTransfer.files && e.dataTransfer.files[0];
      if (file) loadGifSpriteFile(file);
    });
  }
  const prevC = document.getElementById("gif-preview-canvas");
  if (prevC) {
    const getCanvasPos = (ev) => {
      const r = prevC.getBoundingClientRect();
      return { x: (ev.clientX - r.left) * (prevC.width / r.width), y: (ev.clientY - r.top) * (prevC.height / r.height) };
    };
    prevC.addEventListener("pointerdown", (ev) => {
      if (gifPickingKey && gifCustomRawImg) {
        const pt = getCanvasPos(ev);
        const rw = gifCustomRawImg.naturalWidth || gifCustomRawImg.width || 128;
        const rh = gifCustomRawImg.naturalHeight || gifCustomRawImg.height || 128;
        const rc = document.createElement("canvas");
        rc.width = rw; rc.height = rh;
        const rctx = rc.getContext("2d", { willReadFrequently: true });
        rctx.drawImage(gifCustomRawImg, 0, 0, rw, rh);
        const sx = Math.max(0, Math.min(rw - 1, Math.floor((pt.x / prevC.width) * rw)));
        const sy = Math.max(0, Math.min(rh - 1, Math.floor((pt.y / prevC.height) * rh)));
        const px = rctx.getImageData(sx, sy, 1, 1).data;
        gifChromaKeyRgb = [px[0], px[1], px[2]];
        gifPickingKey = false;
        const sw = document.getElementById("gif-key-swatch");
        const btn = document.getElementById("gif-pick-key-btn");
        if (sw) { sw.style.background = "rgb(" + px[0] + "," + px[1] + "," + px[2] + ")"; sw.title = "Key: rgb(" + px[0] + "," + px[1] + "," + px[2] + ")"; }
        if (btn) btn.style.borderColor = "";
        prevC.style.cursor = "";
        refreshCleanedCustomImg();
        if (typeof showToast === "function") showToast("Chroma-keyed rgb(" + px[0] + "," + px[1] + "," + px[2] + ")");
        return;
      }
      const editEl = document.getElementById("gif-show-rig");
      if (!gifCustomRig || !gifLastCanvasGeom || !editEl || !editEl.checked) return;
      const pt = getCanvasPos(ev);
      const g = gifLastCanvasGeom, sx = g.dw / g.sw, sy = g.dh / g.sh;
      let best = -1, bestD = 12;
      for (let i = 0; i < gifCustomRig.pins.length; i++) {
        const rx = g.mirrored ? (gifCustomRig.w - 1 - gifCustomRig.pins[i].x) : gifCustomRig.pins[i].x;
        const px = g.ox + rx * sx;
        const py = g.oy + gifCustomRig.pins[i].y * sy;
        const d = Math.hypot(pt.x - px, pt.y - py);
        if (d < bestD) { bestD = d; best = i; }
      }
      if (best !== -1) { gifDragPinIdx = best; prevC.setPointerCapture(ev.pointerId); }
    });
    prevC.addEventListener("pointermove", (ev) => {
      if (gifDragPinIdx < 0 || !gifCustomRig || !gifLastCanvasGeom) return;
      const pt = getCanvasPos(ev);
      const g = gifLastCanvasGeom;
      let gx = Math.max(0, Math.min(gifCustomRig.w - 1, (pt.x - g.ox) * (g.sw / g.dw)));
      if (g.mirrored) gx = gifCustomRig.w - 1 - gx;
      gifCustomRig.pins[gifDragPinIdx].x = gx;
      gifCustomRig.pins[gifDragPinIdx].y = Math.max(0, Math.min(gifCustomRig.h - 1, (pt.y - g.oy) * (g.sh / g.dh)));
      invalidateGifFrameCache();
      drawGifMakerFrame(prevC.getContext("2d"), prevC.width, prevC.height, 0, 8, true);
    });
    const endDrag = () => {
      if (gifDragPinIdx >= 0 && gifCustomRig) {
        gifCustomRig.geo = computeGeodesicSkinning(gifCustomRig.grid, gifCustomRig.w, gifCustomRig.h, gifCustomRig.pins);
        invalidateGifFrameCache();
      }
      gifDragPinIdx = -1;
    };
    prevC.addEventListener("pointerup", endDrag);
    prevC.addEventListener("pointercancel", endDrag);
  }
  document.addEventListener("keydown", (ev) => {
    const tag = ev.target && ev.target.tagName ? ev.target.tagName.toLowerCase() : "";
    if (tag === "input" || tag === "textarea" || tag === "select") return;
    const sec = document.getElementById("gif-maker");
    if (!sec) return;
    const rect = sec.getBoundingClientRect();
    if (rect.bottom < 0 || rect.top > window.innerHeight) return;
    if (ev.code === "Space") { ev.preventDefault(); toggleGifPlay(); }
    else if (ev.key === "ArrowLeft") { ev.preventDefault(); stepGifFrame(-1); }
    else if (ev.key === "ArrowRight") { ev.preventDefault(); stepGifFrame(1); }
  });
  syncGifMascotSelection();
  updateGifPreviewFps();
}

if (typeof module !== "undefined" && module.exports) {
  module.exports = {
    computeSafeDrawRect,
    getGifPlaybackSequence,
    getGifBgFill,
    buildDeskpetXmlString,
    cleanRgbaPixels,
    detectNativeBlockScale,
    detectSpriteSheetGrid,
    sliceSpriteSheet,
    buildAutoRig,
    computeAnimatedPins,
    computeLimbGaitPose,
    countInteriorHoles,
    buildFrameDiffRgba,
    sealPinholesAndSpecks,
    buildBoneHierarchy,
    computeBoneFkTransforms,
    renderBoneFkFrame,
    renderHarmonicDqsFrame,
    renderFabrikIkFrame,
    renderLayeredInpaintFrame,
    renderComplementaryDynFrame,
    computeBiharmonicWeights,
    solveFabrikChain2D,
    inpaintTorsoBacking,
    evalSpringCycle,
    fitOptimalRotation2D,
    sampleCleanEdgePixel,
    applyParallax25DTransforms,
    renderArap2dFrame,
    renderCleanEdgeFrame,
    renderParallax25dFrame,
    renderSotaHybridFrame,
    renderRiggedFrame,
    extractMedialAxisFeatures,
    computeSpringFollowThrough,
    rgbToOklab,
    assembleApngFromPngFrames,
    encodeGif89a,
  };
}
