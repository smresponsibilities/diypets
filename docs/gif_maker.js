// DIYPets Sprite Sheet Slicer + Inverse Rigid MLS (Schaefer 2006) Auto-Rigger
// + Interactive Draggable Skeleton Pins + Lossless GIF89a & PNG Sprite Sheet Exporter

let gifCustomRig = null;
let gifCustomSheet = null;
let gifCustomRawImg = null;
let gifCustomName = "";
let gifPreviewTick = 0;
let gifPreviewTimer = null;
let gifDragPinIdx = -1;
let gifLastCanvasGeom = null; // { ox, oy, dw, dh, sw, sh }

function isNeutralGrey(r, g, b) {
  const mx = Math.max(r, g, b), mn = Math.min(r, g, b);
  const sat = mx - mn, lum = (r + g + b) / 3;
  return sat <= 14 && lum >= 92 && lum <= 224 && (b - r) <= 6;
}

function cleanRgbaPixels(data, w, h) {
  const out = new Uint8ClampedArray(data);
  const n = w * h;

  const corners = [0, w - 1, (h - 1) * w, (h - 1) * w + (w - 1)];
  const bgSamples = [];
  for (let i = 0; i < corners.length; i++) {
    const p = corners[i] * 4;
    if (out[p + 3] >= 24) bgSamples.push([out[p], out[p + 1], out[p + 2]]);
  }
  const matchOuterBg = (p) => {
    if (out[p + 3] < 32) return true;
    const r = out[p], g = out[p + 1], b = out[p + 2];
    if (bgSamples.length > 0 && isNeutralGrey(r, g, b)) return true;
    for (let i = 0; i < bgSamples.length; i++) {
      const s = bgSamples[i];
      if (Math.abs(r - s[0]) + Math.abs(g - s[1]) + Math.abs(b - s[2]) <= 40) return true;
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

  for (let i = 0; i < n; i++) {
    const p = i * 4, a = out[p + 3];
    if (a < 180 || (a < 250 && isNeutralGrey(out[p], out[p + 1], out[p + 2]))) {
      out[p] = 0; out[p + 1] = 0; out[p + 2] = 0; out[p + 3] = 0;
    } else {
      out[p + 3] = 255;
    }
  }

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
    const isOuter = touchesEmpty > 0 && tail <= Math.max(24, n * 0.04);
    const isTrapped = touchesEmpty === 0 && touchesDark >= 6 && tail <= Math.max(36, n * 0.03);
    if (isOuter || isTrapped) {
      for (let k = 0; k < tail; k++) {
        const p = q2[k] * 4;
        out[p] = 0; out[p + 1] = 0; out[p + 2] = 0; out[p + 3] = 0;
      }
    }
  }

  const alphaCopy = new Uint8Array(n);
  for (let i = 0; i < n; i++) alphaCopy[i] = out[i * 4 + 3];
  for (let y = 1; y < h - 1; y++) {
    for (let x = 1; x < w - 1; x++) {
      const idx = y * w + x;
      if (alphaCopy[idx] === 0) continue;
      if (!alphaCopy[idx - 1] && !alphaCopy[idx + 1] && !alphaCopy[idx - w] && !alphaCopy[idx + w]) {
        const p = idx * 4;
        out[p] = 0; out[p + 1] = 0; out[p + 2] = 0; out[p + 3] = 0;
      }
    }
  }
  return out;
}

function detectNativeBlockScale(rgba, w, h) {
  const scales = [4, 3, 2];
  for (let si = 0; si < scales.length; si++) {
    const s = scales[si];
    if (w % s !== 0 || h % s !== 0 || w / s < 16 || h / s < 16) continue;
    let checked = 0, uniform = 0;
    for (let by = 0; by < h; by += s) {
      for (let bx = 0; bx < w; bx += s) {
        const p0 = (by * w + bx) * 4;
        let hasOpaque = false, same = true;
        for (let dy = 0; dy < s && same; dy++) {
          for (let dx = 0; dx < s; dx++) {
            const p = ((by + dy) * w + (bx + dx)) * 4;
            if (rgba[p + 3] > 0) hasOpaque = true;
            if (
              Math.abs(rgba[p] - rgba[p0]) > 14 || Math.abs(rgba[p + 1] - rgba[p0 + 1]) > 14 ||
              Math.abs(rgba[p + 2] - rgba[p0 + 2]) > 14 || Math.abs(rgba[p + 3] - rgba[p0 + 3]) > 40
            ) { same = false; break; }
          }
        }
        if (hasOpaque) { checked++; if (same) uniform++; }
      }
    }
    if (checked > 20 && (uniform / checked) >= 0.88) return s;
  }
  return 1;
}

function downsampleToNativeGrid(rgba, w, h) {
  let block = detectNativeBlockScale(rgba, w, h);
  let nw = Math.floor(w / block), nh = Math.floor(h / block);
  while (Math.max(nw, nh) > 64) {
    block *= 2;
    nw = Math.max(16, Math.floor(w / block));
    nh = Math.max(16, Math.floor(h / block));
  }
  const grid = new Uint8ClampedArray(nw * nh * 4);
  const off = Math.floor(block / 2);
  for (let y = 0; y < nh; y++) {
    const sy = Math.min(h - 1, y * block + off);
    for (let x = 0; x < nw; x++) {
      const sx = Math.min(w - 1, x * block + off);
      const sp = (sy * w + sx) * 4, dp = (y * nw + x) * 4;
      grid[dp] = rgba[sp]; grid[dp + 1] = rgba[sp + 1]; grid[dp + 2] = rgba[sp + 2];
      grid[dp + 3] = rgba[sp + 3] >= 128 ? 255 : 0;
    }
  }
  return { grid, w: nw, h: nh, block };
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

// =========================================================================
// Inverse Rigid Moving Least Squares (Schaefer et al., SIGGRAPH 2006)
// + Skeleton Pin Auto-Placement (horns-safe lower-body aspect & paw clustering)
// =========================================================================
function findColumnMassCenter(grid, w, x0, x1, y0, y1, fallbackX, fallbackY) {
  let sx = 0, sy = 0, cnt = 0;
  for (let y = Math.max(0, Math.floor(y0)); y <= Math.min(Math.floor(y1), (grid.length / (4 * w)) - 1); y++) {
    for (let x = Math.max(0, Math.floor(x0)); x <= Math.min(w - 1, Math.floor(x1)); x++) {
      if (grid[(y * w + x) * 4 + 3] > 0) { sx += x; sy += y; cnt++; }
    }
  }
  return cnt > 0 ? { x: sx / cnt, y: sy / cnt } : { x: fallbackX, y: fallbackY };
}

function buildAutoRig(rawRgba, rawW, rawH, cleanBg, modeHint) {
  const cleaned = cleanBg ? cleanRgbaPixels(rawRgba, rawW, rawH) : new Uint8ClampedArray(rawRgba);
  const { grid, w, h } = downsampleToNativeGrid(cleaned, rawW, rawH);
  const bbox = findBbox(grid, w, h);

  // Measure facing direction using top 45% mass
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

  // Compute LOWER-BODY bounding box (bottom 62% of height) so tall horns/ears on Goat never distort aspect ratio!
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

  // Bottom-up paw-gap scan
  const scanTop = bbox.minY + Math.floor(bbox.bh * 0.58);
  let maxRuns = 1, highestGapY = -1;
  for (let y = bbox.maxY - 1; y >= scanTop; y--) {
    let runs = 0, runLen = 0;
    for (let x = bbox.minX; x <= bbox.maxX; x++) {
      if (grid[(y * w + x) * 4 + 3] > 0) { runLen++; }
      else { if (runLen >= 2) runs++; runLen = 0; }
    }
    if (runLen >= 2) runs++;
    if (runs > maxRuns) maxRuns = runs;
    if (runs >= 2) highestGapY = y;
  }

  let bodyType = modeHint && modeHint !== "auto" ? modeHint : "quadruped";
  if (!modeHint || modeHint === "auto") {
    if (maxRuns >= 3 || (maxRuns >= 2 && lowerAspect >= 1.12)) bodyType = "quadruped";
    else if (maxRuns >= 2) bodyType = "biped";
    else bodyType = "blob";
  }

  const hipY = highestGapY !== -1
    ? Math.max(bbox.minY + Math.floor(bbox.bh * 0.62), highestGapY - Math.max(2, Math.round(bbox.bh * 0.06)))
    : bbox.minY + Math.floor(bbox.bh * 0.70);

  // Place 7 anatomical control pins (Head, Chest, Pelvis, Tail, Front-A, Front-B, Back-A, Back-B)
  // User can drag ANY of these pins interactively on the canvas!
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

  // Locate paw X coordinates cleanly from actual bottom pixels in each foot zone
  const findPawX = (t0, t1, fallbackT) => {
    const xA = Math.min(atRelX(t0), atRelX(t1));
    const xB = Math.max(atRelX(t0), atRelX(t1));
    const c = findColumnMassCenter(grid, w, xA, xB, Math.max(hipY, bbox.maxY - 5), bbox.maxY, atRelX(fallbackT), bbox.maxY - 1);
    return c.x;
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
      { id: "fl", label: "FL", color: "#50f082", x: findPawX(0.10, 0.34, 0.22), y: pawY },
      { id: "fr", label: "FR", color: "#ff7373", x: findPawX(0.30, 0.52, 0.40), y: pawY },
      { id: "bl", label: "BL", color: "#ff7373", x: findPawX(0.54, 0.78, 0.68), y: pawY },
      { id: "br", label: "BR", color: "#50f082", x: findPawX(0.74, 0.96, 0.85), y: pawY }
    );
  } else if (bodyType === "biped") {
    pins.push(
      { id: "fl", label: "L-Foot", color: "#50f082", x: findPawX(0.18, 0.50, 0.35), y: pawY },
      { id: "br", label: "R-Foot", color: "#ff7373", x: findPawX(0.50, 0.84, 0.66), y: pawY }
    );
  } else {
    pins.push(
      { id: "fl", label: "BaseL", color: "#50f082", x: atRelX(0.28), y: pawY },
      { id: "br", label: "BaseR", color: "#50f082", x: atRelX(0.72), y: pawY }
    );
  }

  return { grid, w, h, bbox, hipY, facesLeft, bodyType, maxRuns, pins };
}

// Compute displaced target pins `q` from rest pins `p` for a given animation frame
function computeAnimatedPins(rig, fx, frameIdx, totalFrames) {
  const { bbox, facesLeft, bodyType, pins } = rig;
  const phase = (frameIdx / totalFrames) * Math.PI * 2;
  const dir = facesLeft ? -1 : 1;
  const stride = Math.max(2.0, bbox.bw * 0.095);
  const liftMax = Math.max(1.8, (bbox.maxY - rig.hipY) * 0.28);

  return pins.map((pin) => {
    let dx = 0, dy = 0;
    if (fx === "walk") {
      const sA = Math.sin(phase);
      const sB = Math.sin(phase + Math.PI);
      const lA = Math.max(0, Math.cos(phase));
      const lB = Math.max(0, Math.cos(phase + Math.PI));
      const bob = -Math.abs(Math.cos(phase)) * Math.max(0.8, bbox.bh * 0.032);
      if (pin.id === "head") {
        dy = -Math.abs(Math.cos(phase - 0.8)) * Math.max(1.0, bbox.bh * 0.042);
        dx = Math.sin(phase - 0.6) * 1.1 * dir;
      } else if (pin.id === "chest") {
        dy = bob;
      } else if (pin.id === "pelvis") {
        dy = bob * 0.9;
      } else if (pin.id === "tail") {
        dy = bob + Math.sin(phase * 2) * 1.6;
        dx = -Math.cos(phase) * 0.9 * dir;
      } else if (bodyType !== "blob" && (pin.id === "fl" || pin.id === "br")) {
        // Diagonal pair A swings while B stays planted at dy = 0 (strict ground contact!)
        dx = sA * stride;
        dy = -lA * liftMax;
      } else if (bodyType !== "blob" && (pin.id === "fr" || pin.id === "bl")) {
        dx = sB * stride;
        dy = -lB * liftMax;
      }
    } else if (fx === "run") {
      const runS = Math.max(2.8, bbox.bw * 0.13);
      const runL = Math.max(2.4, (bbox.maxY - rig.hipY) * 0.36);
      const airBob = -Math.max(0, Math.sin(phase)) * Math.max(1.8, bbox.bh * 0.06);
      if (pin.id === "head") { dx = dir * 1.5; dy = airBob; }
      else if (pin.id === "chest" || pin.id === "pelvis") { dy = airBob; dx = dir * 0.6; }
      else if (pin.id === "tail") { dy = airBob + Math.sin(phase * 2) * 2.0; dx = -dir * 1.2; }
      else if (pin.id === "fl") { dx = Math.sin(phase) * runS; dy = -Math.max(0, Math.cos(phase)) * runL; }
      else if (pin.id === "fr") { dx = Math.sin(phase + 0.55) * runS; dy = -Math.max(0, Math.cos(phase + 0.55)) * runL; }
      else if (pin.id === "bl") { dx = Math.sin(phase + Math.PI * 0.85) * runS; dy = -Math.max(0, Math.cos(phase + Math.PI * 0.85)) * runL; }
      else if (pin.id === "br") { dx = Math.sin(phase + Math.PI * 1.15) * runS; dy = -Math.max(0, Math.cos(phase + Math.PI * 1.15)) * runL; }
    } else if (fx === "breathe") {
      const ex = (Math.sin(phase) * 0.5 + 0.5) * Math.max(1.0, bbox.bh * 0.045);
      if (pin.id === "head" || pin.id === "chest") dy = ex;
      else if (pin.id === "pelvis") dy = ex * 0.65;
      else if (pin.id === "tail") dy = ex * 0.8;
    } else if (fx === "hop") {
      const u = frameIdx / totalFrames;
      if (u < 0.24) {
        // Anticipation crouch (paws locked at dy = 0!)
        if (pin.id === "head") dy = 2.4;
        else if (pin.id === "chest" || pin.id === "pelvis" || pin.id === "tail") dy = 1.6;
      } else if (u < 0.68) {
        const air = Math.sin(((u - 0.24) / 0.44) * Math.PI);
        const hLift = air * Math.max(4.0, bbox.bh * 0.19);
        dy = -hLift;
        if (pin.id === "head") dy -= air * 1.2;
        if (pin.id === "fl" || pin.id === "fr") { dx = dir * air * 1.6; dy += air * 1.2; }
        if (pin.id === "bl" || pin.id === "br") { dx = -dir * air * 1.6; dy += air * 1.2; }
      } else {
        if (pin.id === "head") dy = 1.4;
        else if (pin.id === "chest" || pin.id === "pelvis") dy = 1.0;
      }
    } else if (fx === "wave") {
      if (pin.id === "tail") { dy = Math.sin(phase) * 2.6; dx = Math.cos(phase) * 1.6; }
      else if (pin.id === "head") { dy = Math.sin(phase + 1.1) * 1.1; }
    }
    return { x: pin.x + dx, y: pin.y + dy };
  });
}

// Closed-form 2D Rigid Moving Least Squares (Schaefer et al., SIGGRAPH 2006)
// Maps destination pixel `v` INVERSELY back to source pixel coordinates (ZERO holes, ZERO tears!)
function evalRigidMlsInverse(vx, vy, srcPins, dstPins) {
  const n = dstPins.length;
  let wSum = 0, qxStar = 0, qyStar = 0, pxStar = 0, pyStar = 0;
  const wArr = new Float64Array(n);

  for (let i = 0; i < n; i++) {
    const dx = vx - dstPins[i].x;
    const dy = vy - dstPins[i].y;
    const dist2 = dx * dx + dy * dy;
    if (dist2 < 1e-6) return { x: srcPins[i].x, y: srcPins[i].y };
    // Alpha = 1.5 gives crisp local limb isolation while keeping smooth C^inf joints
    const wi = 1.0 / Math.pow(dist2, 1.5);
    wArr[i] = wi;
    wSum += wi;
    qxStar += wi * dstPins[i].x;
    qyStar += wi * dstPins[i].y;
    pxStar += wi * srcPins[i].x;
    pyStar += wi * srcPins[i].y;
  }
  qxStar /= wSum; qyStar /= wSum;
  pxStar /= wSum; pyStar /= wSum;

  const dvx = vx - qxStar;
  const dvy = vy - qyStar;
  const dvLen = Math.hypot(dvx, dvy);
  if (dvLen < 1e-6) return { x: pxStar, y: pyStar };

  // Compute linear rigid vector f_r(v) = |v - q*| * fr / |fr| + p*
  let frx = 0, fry = 0;
  for (let i = 0; i < n; i++) {
    const qhx = dstPins[i].x - qxStar;
    const qhy = dstPins[i].y - qyStar;
    const phx = srcPins[i].x - pxStar;
    const phy = srcPins[i].y - pyStar;
    const wi = wArr[i];
    // 2x2 A_i * [dvx, dvy]^T where A_i = wi * [[qhx, qhy],[qhy, -qhx]] * [[dvx, dvy],[dvy, -dvx]]^T
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

function renderRiggedFrame(rig, fx, frameIdx, totalFrames) {
  const { grid, w, h, bbox, pins } = rig;
  const dstPins = computeAnimatedPins(rig, fx, frameIdx, totalFrames);
  const out = new Uint8ClampedArray(w * h * 4);

  // Restrict evaluation to padded sprite bounding box for fast 60fps preview
  const pad = Math.max(6, Math.ceil(bbox.bw * 0.22));
  const y0 = Math.max(0, bbox.minY - pad);
  const y1 = Math.min(h - 1, bbox.maxY + 2);
  const x0 = Math.max(0, bbox.minX - pad);
  const x1 = Math.min(w - 1, bbox.maxX + pad);

  for (let y = y0; y <= y1; y++) {
    for (let x = x0; x <= x1; x++) {
      const s = evalRigidMlsInverse(x, y, pins, dstPins);
      const sx = Math.round(s.x);
      const sy = Math.round(s.y);
      if (sx < 0 || sx >= w || sy < 0 || sy >= h) continue;
      const sp = (sy * w + sx) * 4;
      if (grid[sp + 3] === 0) continue;
      const dp = (y * w + x) * 4;
      out[dp] = grid[sp]; out[dp + 1] = grid[sp + 1]; out[dp + 2] = grid[sp + 2]; out[dp + 3] = 255;
    }
  }

  // Strict ground-baseline alignment for grounded cycles (zero vertical foot jitter!)
  if (fx === "walk" || fx === "breathe" || fx === "wave") {
    let curFoot = -1;
    for (let y = h - 1; y >= 0 && curFoot === -1; y--) {
      for (let x = 0; x < w; x++) {
        if (out[(y * w + x) * 4 + 3] > 0) { curFoot = y; break; }
      }
    }
    if (curFoot !== -1 && curFoot !== bbox.maxY) {
      const shift = bbox.maxY - curFoot;
      if (Math.abs(shift) <= 4) {
        const aligned = new Uint8ClampedArray(w * h * 4);
        for (let y = 0; y < h; y++) {
          const ty = y + shift;
          if (ty < 0 || ty >= h) continue;
          for (let x = 0; x < w; x++) {
            const sp = (y * w + x) * 4, dp = (ty * w + x) * 4;
            aligned[dp] = out[sp]; aligned[dp + 1] = out[sp + 1]; aligned[dp + 2] = out[sp + 2]; aligned[dp + 3] = out[sp + 3];
          }
        }
        return aligned;
      }
    }
  }
  return out;
}

// =========================================================================
// UI Controls, Interactive Draggable Pins, GIF89a & PNG Sprite Sheet Export
// =========================================================================
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
    gifCustomRig = buildAutoRig(raw, w, h, cleanBg, bodySel ? bodySel.value : "auto");
    if (st) {
      st.textContent = "RIGID MLS (" + gifCustomRig.bodyType.toUpperCase() + ", " + gifCustomRig.w + "x" + gifCustomRig.h + " GRID)";
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
  if (st) st.textContent = "LOSSLESS GIF89A & PNG SHEET";
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

function drawNativeRgbaIntegerScaled(targetCtx, rgba, sw, sh, outW, outH, padBottom, storeGeom) {
  const off = document.createElement("canvas");
  off.width = sw; off.height = sh;
  off.getContext("2d").putImageData(new ImageData(rgba, sw, sh), 0, 0);

  const availW = Math.max(sw, outW - 2);
  const availH = Math.max(sh, outH - padBottom - 2);
  const intScale = Math.max(1, Math.floor(Math.min(availW / sw, availH / sh)));
  let dw = sw * intScale, dh = sh * intScale;
  if (dw > outW || dh > outH - padBottom) {
    const r = Math.min((outW - 4) / sw, (outH - padBottom - 4) / sh);
    dw = Math.max(1, Math.round(sw * r));
    dh = Math.max(1, Math.round(sh * r));
  }
  const ox = Math.floor((outW - dw) / 2);
  const oy = Math.max(0, outH - padBottom - dh);
  targetCtx.imageSmoothingEnabled = false;
  targetCtx.drawImage(off, 0, 0, sw, sh, ox, oy, dw, dh);
  if (storeGeom) gifLastCanvasGeom = { ox, oy, dw, dh, sw, sh };
}

function drawPinOverlay(targetCtx, rig, geom) {
  if (!rig || !geom) return;
  const sx = geom.dw / geom.sw, sy = geom.dh / geom.sh;
  const toC = (p) => ({ x: geom.ox + p.x * sx, y: geom.oy + p.y * sy });

  // Draw bone lines connecting spine & paws
  const byId = {};
  for (let i = 0; i < rig.pins.length; i++) byId[rig.pins[i].id] = toC(rig.pins[i]);
  const edges = [["head", "chest"], ["chest", "pelvis"], ["pelvis", "tail"], ["chest", "fl"], ["chest", "fr"], ["pelvis", "bl"], ["pelvis", "br"]];
  targetCtx.save();
  targetCtx.strokeStyle = "rgba(255,255,255,0.75)";
  targetCtx.lineWidth = 1.5;
  for (let i = 0; i < edges.length; i++) {
    const a = byId[edges[i][0]], b = byId[edges[i][1]];
    if (!a || !b) continue;
    targetCtx.beginPath(); targetCtx.moveTo(a.x, a.y); targetCtx.lineTo(b.x, b.y); targetCtx.stroke();
  }
  for (let i = 0; i < rig.pins.length; i++) {
    const pt = toC(rig.pins[i]);
    targetCtx.beginPath();
    targetCtx.arc(pt.x, pt.y, i === gifDragPinIdx ? 5.5 : 4.2, 0, Math.PI * 2);
    targetCtx.fillStyle = rig.pins[i].color;
    targetCtx.fill();
    targetCtx.strokeStyle = "#0b0e12";
    targetCtx.lineWidth = 1.5;
    targetCtx.stroke();
  }
  targetCtx.restore();
}

function drawGifMakerFrame(targetCtx, w, h, frameIdx, totalFrames, allowOverlay) {
  const petId = document.getElementById("gif-pet-select").value;
  const fx = document.getElementById("gif-fx-select").value;
  const bgMode = document.getElementById("gif-bg-select").value;
  const caption = (document.getElementById("gif-caption").value || "").trim();
  const editPinsEl = document.getElementById("gif-show-rig");
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
  }

  const padBottom = bgMode === "card" ? Math.round(8 * amp) : Math.round(2 * amp);
  if (gifCustomSheet && gifCustomSheet.activeIndices.length > 0) {
    const idx = gifCustomSheet.activeIndices[frameIdx % gifCustomSheet.activeIndices.length];
    const tile = gifCustomSheet.frames[idx] || gifCustomSheet.frames[0];
    drawNativeRgbaIntegerScaled(targetCtx, tile.rgba, tile.w, tile.h, w, h, padBottom, false);
    return;
  }
  if (gifCustomRig) {
    // When EDIT PINS is checked, pause on rest pose so the user can drag joint pins cleanly!
    const frameRgba = editPins ? gifCustomRig.grid : renderRiggedFrame(gifCustomRig, fx, frameIdx, totalFrames);
    drawNativeRgbaIntegerScaled(targetCtx, frameRgba, gifCustomRig.w, gifCustomRig.h, w, h, padBottom, allowOverlay);
    if (editPins) drawPinOverlay(targetCtx, gifCustomRig, gifLastCanvasGeom);
    return;
  }

  if (typeof sprites !== "undefined" && sprites[petId] && sprites[petId].complete && sprites[petId].naturalWidth > 0) {
    const sp = sprites[petId];
    const fw = Math.floor(sp.naturalWidth / 9), fh = sp.naturalHeight;
    const fCol = (fx === "walk" || fx === "run") ? (1 + (frameIdx % 4)) : 0;
    const dy = fx === "hop" ? -Math.round(Math.sin((frameIdx / totalFrames) * Math.PI) * 14 * amp) : 0;
    const box = w - Math.round((bgMode === "card" ? 24 : 8) * amp);
    targetCtx.drawImage(sp, fCol * fw, 0, fw, fh, Math.floor((w - box) / 2), h - padBottom - box + dy, box, box);
  }
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

function buildGifPalette(framesRgba, transparent) {
  const uniqueMap = new Map(), counts = [];
  for (let f = 0; f < framesRgba.length; f++) {
    const rgba = framesRgba[f];
    for (let p = 0; p < rgba.length; p += 4) {
      if (transparent && rgba[p + 3] < 128) continue;
      const r = rgba[p], g = rgba[p + 1], b = rgba[p + 2], key = (r << 16) | (g << 8) | b;
      const idx = uniqueMap.get(key);
      if (idx === undefined) { uniqueMap.set(key, counts.length); counts.push([r, g, b, 1]); }
      else counts[idx][3]++;
    }
  }
  const palette = [[0, 0, 0]];
  if (counts.length <= 255) {
    for (let i = 0; i < counts.length; i++) palette.push([counts[i][0], counts[i][1], counts[i][2]]);
    return palette;
  }
  const boxes = [counts];
  while (boxes.length < 255) {
    let splitIdx = -1, bestRange = -1, splitChan = 0;
    for (let i = 0; i < boxes.length; i++) {
      const box = boxes[i];
      if (box.length < 2) continue;
      let minR = 255, maxR = 0, minG = 255, maxG = 0, minB = 255, maxB = 0;
      for (let j = 0; j < box.length; j++) {
        const c = box[j];
        if (c[0] < minR) minR = c[0]; if (c[0] > maxR) maxR = c[0];
        if (c[1] < minG) minG = c[1]; if (c[1] > maxG) maxG = c[1];
        if (c[2] < minB) minB = c[2]; if (c[2] > maxB) maxB = c[2];
      }
      const rR = maxR - minR, rG = maxG - minG, rB = maxB - minB, m = Math.max(rR, rG, rB);
      if (m > bestRange) { bestRange = m; splitIdx = i; splitChan = m === rG ? 1 : (m === rR ? 0 : 2); }
    }
    if (splitIdx === -1 || bestRange <= 0) break;
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

function encodeGif89a(framesRgba, w, h, delayCs, transparent) {
  const palette = buildGifPalette(framesRgba, transparent);
  const lookup = new Map();
  for (let i = 1; i < palette.length; i++) {
    const c = palette[i];
    lookup.set((c[0] << 16) | (c[1] << 8) | c[2], i);
  }
  const indexedFrames = [];
  for (let f = 0; f < framesRgba.length; f++) {
    const rgba = framesRgba[f], idxBuf = new Uint8Array(w * h);
    for (let p = 0; p < w * h; p++) {
      if (transparent && rgba[p * 4 + 3] < 128) { idxBuf[p] = 0; continue; }
      const r = rgba[p * 4], g = rgba[p * 4 + 1], b = rgba[p * 4 + 2], key = (r << 16) | (g << 8) | b;
      let ci = lookup.get(key);
      if (ci === undefined) {
        let best = 1, bestD = 1e9;
        for (let j = 1; j < palette.length; j++) {
          const dr = r - palette[j][0], dg = g - palette[j][1], db = b - palette[j][2];
          const d = dr * dr * 2 + dg * dg * 4 + db * db * 3;
          if (d < bestD) { bestD = d; best = j; }
        }
        ci = best; lookup.set(key, ci);
      }
      idxBuf[p] = ci;
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
    bytes.push(0x21, 0xF9, 0x04, transparent ? 0x09 : 0x08);
    pushU16(delayCs); bytes.push(0x00, 0x00, 0x2C);
    pushU16(0); pushU16(0); pushU16(w); pushU16(h); bytes.push(0x00, 8);
    const lzw = lzwEncodeGif(indexedFrames[f], 8);
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

    if (!gifCustomRig && !gifCustomSheet && (!sprites[petId] || !sprites[petId].complete || sprites[petId].naturalWidth === 0)) {
      statusEl.textContent = "WAIT: SPRITE STILL LOADING...";
      return;
    }
    const totalFrames = getGifTotalFrames(fx);
    const frames = [];
    for (let i = 0; i < totalFrames; i++) {
      drawGifMakerFrame(octx, size, size, i, totalFrames, false);
      frames.push(octx.getImageData(0, 0, size, size).data);
    }
    const gifBytes = encodeGif89a(frames, size, size, Math.max(4, Math.round(100 / fps)), transparent);
    const blob = new Blob([gifBytes], { type: "image/gif" });
    const slug = ((gifCustomRig || gifCustomSheet) && gifCustomName) ? gifCustomName : petId;
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = slug + "-" + (gifCustomSheet ? "sheet" : fx) + "-" + size + "px.gif";
    document.body.appendChild(a); a.click(); a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 2500);
    statusEl.textContent = "EXPORTED " + (gifBytes.length / 1024).toFixed(1) + " KB GIF (" + size + "px, " + totalFrames + "f)";
    if (typeof showToast === "function") showToast("Exported " + a.download);
  } catch (err) {
    console.error("GIF export error:", err);
    statusEl.textContent = "ERROR: USE HTTPS OR UPLOAD A LOCAL FILE";
  }
}

// Export all generated frames as a crisp horizontal PNG Sprite Sheet (ready for Aseprite / Godot / Inspection!)
function exportMascotSpriteSheet() {
  const statusEl = document.getElementById("gif-status");
  try {
    const size = Math.max(64, Math.min(256, Number(document.getElementById("gif-size-select").value) || 128));
    const fx = document.getElementById("gif-fx-select").value;
    const petId = document.getElementById("gif-pet-select").value;
    const totalFrames = getGifTotalFrames(fx);

    const tileC = document.createElement("canvas");
    tileC.width = size; tileC.height = size;
    const tctx = tileC.getContext("2d");

    const sheetC = document.createElement("canvas");
    sheetC.width = size * totalFrames; sheetC.height = size;
    const sctx = sheetC.getContext("2d");
    sctx.imageSmoothingEnabled = false;

    for (let i = 0; i < totalFrames; i++) {
      drawGifMakerFrame(tctx, size, size, i, totalFrames, false);
      sctx.drawImage(tileC, i * size, 0);
    }

    sheetC.toBlob((blob) => {
      if (!blob) return;
      const slug = ((gifCustomRig || gifCustomSheet) && gifCustomName) ? gifCustomName : petId;
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = slug + "-" + (gifCustomSheet ? "sheet" : fx) + "-" + totalFrames + "x1-" + size + "px.png";
      document.body.appendChild(a); a.click(); a.remove();
      setTimeout(() => URL.revokeObjectURL(a.href), 2500);
      statusEl.textContent = "EXPORTED " + totalFrames + "x1 PNG SHEET (" + (blob.size / 1024).toFixed(1) + " KB)";
      if (typeof showToast === "function") showToast("Exported PNG sprite sheet (" + totalFrames + " frames)");
    }, "image/png");
  } catch (err) {
    console.error("Sheet export error:", err);
    statusEl.textContent = "ERROR EXPORTING PNG SHEET";
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
    const total = getGifTotalFrames(fx);
    gifPreviewTick = (gifPreviewTick + 1) % total;
    drawGifMakerFrame(prevC.getContext("2d"), prevC.width, prevC.height, gifPreviewTick, total, true);
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
      const editEl = document.getElementById("gif-show-rig");
      if (!gifCustomRig || !gifLastCanvasGeom || !editEl || !editEl.checked) return;
      const pt = getCanvasPos(ev);
      const g = gifLastCanvasGeom, sx = g.dw / g.sw, sy = g.dh / g.sh;
      let best = -1, bestD = 12;
      for (let i = 0; i < gifCustomRig.pins.length; i++) {
        const px = g.ox + gifCustomRig.pins[i].x * sx;
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
      gifCustomRig.pins[gifDragPinIdx].x = Math.max(0, Math.min(gifCustomRig.w - 1, (pt.x - g.ox) * (g.sw / g.dw)));
      gifCustomRig.pins[gifDragPinIdx].y = Math.max(0, Math.min(gifCustomRig.h - 1, (pt.y - g.oy) * (g.sh / g.dh)));
      drawGifMakerFrame(prevC.getContext("2d"), prevC.width, prevC.height, 0, 8, true);
    });
    const endDrag = () => { gifDragPinIdx = -1; };
    prevC.addEventListener("pointerup", endDrag);
    prevC.addEventListener("pointercancel", endDrag);
  }
  syncGifMascotSelection();
  updateGifPreviewFps();
}

if (typeof module !== "undefined" && module.exports) {
  module.exports = {
    cleanRgbaPixels,
    detectSpriteSheetGrid,
    sliceSpriteSheet,
    buildAutoRig,
    renderRiggedFrame,
    encodeGif89a,
  };
}
