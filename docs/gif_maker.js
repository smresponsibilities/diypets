// DIYPets Native-Grid Pixel-Art Auto-Rigger & Lossless GIF89a Maker
// Pipeline:
//  1. Clean outer BG, trapped interior grey pockets, and anti-aliased fringe pixels.
//  2. Detect native pixel block scale (1x/2x/3x/4x) and work on the exact 1px grid (zero mixels).
//  3. Auto-segment silhouette into Head, Torso, Tail, and Individual Legs (quadruped / biped / blob).
//  4. Animate each part independently with integer Paeth shear, hip-seam healing, and 1px RPG breathing.
//  5. Upscale by exact integer scale and encode verified GIF89a (exact palette or Median-Cut).

let gifCustomRig = null;
let gifCustomRawImg = null;
let gifCustomName = "";
let gifPreviewTick = 0;
let gifPreviewTimer = null;

function isNeutralGrey(r, g, b) {
  const mx = Math.max(r, g, b);
  const mn = Math.min(r, g, b);
  const sat = mx - mn;
  const lum = (r + g + b) / 3;
  // Checkerboard / matte grey range (explicitly excludes bright white fur lum >= 230 and cool-tinted shadows b > r+7)
  return sat <= 14 && lum >= 92 && lum <= 224 && (b - r) <= 6;
}

function cleanRgbaPixels(data, w, h) {
  const out = new Uint8ClampedArray(data);
  const n = w * h;

  // Pass 1: Border flood-fill removal (strips solid or 2-tone white/grey checkerboards from outside in)
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

  // Pass 2: Hard alpha matte (eliminates translucent grey anti-aliased edges completely)
  for (let i = 0; i < n; i++) {
    const p = i * 4;
    const a = out[p + 3];
    if (a < 180) {
      out[p] = 0; out[p + 1] = 0; out[p + 2] = 0; out[p + 3] = 0;
    } else if (a < 250 && isNeutralGrey(out[p], out[p + 1], out[p + 2])) {
      out[p] = 0; out[p + 1] = 0; out[p + 2] = 0; out[p + 3] = 0;
    } else {
      out[p + 3] = 255;
    }
  }

  // Pass 3: Strip trapped neutral-grey matte/checkerboard pockets touching transparency OR small enclosed grey islands
  const visPocket = new Uint8Array(n);
  const q2 = new Int32Array(n);
  for (let i = 0; i < n; i++) {
    if (visPocket[i] || out[i * 4 + 3] === 0) continue;
    const p0 = i * 4;
    if (!isNeutralGrey(out[p0], out[p0 + 1], out[p0 + 2])) continue;
    let head = 0, tail = 0;
    let touchesEmpty = 0;
    let touchesDark = 0;
    visPocket[i] = 1;
    q2[tail++] = i;
    while (head < tail) {
      const cur = q2[head++];
      const cx = cur % w, cy = (cur / w) | 0;
      const nbrs = [
        cx > 0 ? cur - 1 : -1,
        cx + 1 < w ? cur + 1 : -1,
        cy > 0 ? cur - w : -1,
        cy + 1 < h ? cur + w : -1,
      ];
      for (let k = 0; k < 4; k++) {
        const nb = nbrs[k];
        if (nb < 0) { touchesEmpty++; continue; }
        const np = nb * 4;
        if (out[np + 3] === 0) { touchesEmpty++; continue; }
        if (isNeutralGrey(out[np], out[np + 1], out[np + 2])) {
          if (!visPocket[nb]) { visPocket[nb] = 1; q2[tail++] = nb; }
        } else {
          const lum = (out[np] + out[np + 1] + out[np + 2]) / 3;
          if (lum < 85) touchesDark++;
        }
      }
    }
    // Clear neutral-grey fringes touching transparency or trapped checkerboard pockets bounded by outline
    const isOuterFringe = touchesEmpty > 0 && tail <= Math.max(24, (w * h) * 0.04);
    const isTrappedPocket = touchesEmpty === 0 && touchesDark >= 6 && tail <= Math.max(36, (w * h) * 0.03);
    if (isOuterFringe || isTrappedPocket) {
      for (let k = 0; k < tail; k++) {
        const p = q2[k] * 4;
        out[p] = 0; out[p + 1] = 0; out[p + 2] = 0; out[p + 3] = 0;
      }
    }
  }

  // Pass 4: Despeckle isolated 1px orphan fringe pixels
  const alphaCopy = new Uint8Array(n);
  for (let i = 0; i < n; i++) alphaCopy[i] = out[i * 4 + 3];
  for (let y = 1; y < h - 1; y++) {
    for (let x = 1; x < w - 1; x++) {
      const idx = y * w + x;
      if (alphaCopy[idx] === 0) continue;
      let solidN = 0;
      if (alphaCopy[idx - 1]) solidN++;
      if (alphaCopy[idx + 1]) solidN++;
      if (alphaCopy[idx - w]) solidN++;
      if (alphaCopy[idx + w]) solidN++;
      if (solidN === 0) {
        const p = idx * 4;
        out[p] = 0; out[p + 1] = 0; out[p + 2] = 0; out[p + 3] = 0;
      }
    }
  }

  return out;
}

function detectNativeBlockScale(rgba, w, h) {
  // Measure how cleanly pixels align to 2x2 / 3x3 / 4x4 uniform blocks
  const scales = [4, 3, 2];
  for (let si = 0; si < scales.length; si++) {
    const s = scales[si];
    if (w % s !== 0 || h % s !== 0 || w / s < 16 || h / s < 16) continue;
    let checked = 0, uniform = 0;
    for (let by = 0; by < h; by += s) {
      for (let bx = 0; bx < w; bx += s) {
        const p0 = (by * w + bx) * 4;
        let hasOpaque = false;
        let same = true;
        for (let dy = 0; dy < s && same; dy++) {
          for (let dx = 0; dx < s; dx++) {
            const p = ((by + dy) * w + (bx + dx)) * 4;
            if (rgba[p + 3] > 0) hasOpaque = true;
            if (
              Math.abs(rgba[p] - rgba[p0]) > 14 ||
              Math.abs(rgba[p + 1] - rgba[p0 + 1]) > 14 ||
              Math.abs(rgba[p + 2] - rgba[p0 + 2]) > 14 ||
              Math.abs(rgba[p + 3] - rgba[p0 + 3]) > 40
            ) {
              same = false;
              break;
            }
          }
        }
        if (hasOpaque) {
          checked++;
          if (same) uniform++;
        }
      }
    }
    if (checked > 20 && (uniform / checked) >= 0.86) return s;
  }
  return 1;
}

function downsampleToNativeGrid(rgba, w, h) {
  let block = detectNativeBlockScale(rgba, w, h);
  let nw = Math.floor(w / block);
  let nh = Math.floor(h / block);
  // Cap working pixel grid around 48-64px max so big uploaded photos/AI PNGs become crisp pixel art
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
      const sp = (sy * w + sx) * 4;
      const dp = (y * nw + x) * 4;
      grid[dp] = rgba[sp];
      grid[dp + 1] = rgba[sp + 1];
      grid[dp + 2] = rgba[sp + 2];
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
        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
        if (y < minY) minY = y;
        if (y > maxY) maxY = y;
      }
    }
  }
  if (maxX < minX) return { minX: 0, minY: 0, maxX: w - 1, maxY: h - 1, bw: w, bh: h };
  return { minX, minY, maxX, maxY, bw: maxX - minX + 1, bh: maxY - minY + 1 };
}

function detectDarkOutlineColor(grid, w, h) {
  let best = [24, 24, 32];
  let bestLum = 999;
  for (let y = 1; y < h - 1; y++) {
    for (let x = 1; x < w - 1; x++) {
      const idx = y * w + x;
      const p = idx * 4;
      if (grid[p + 3] === 0) continue;
      // Only sample exterior boundary pixels
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

// Anatomical Auto-Rigger: segments native pixel grid into:
//   0 = empty, 1 = torso, 2 = head, 3 = tail, 10..13 = individual legs (front-near, front-far, back-near, back-far)
function buildAutoRig(rawRgba, rawW, rawH, cleanBg, modeHint) {
  const cleaned = cleanBg ? cleanRgbaPixels(rawRgba, rawW, rawH) : new Uint8ClampedArray(rawRgba);
  const { grid, w, h } = downsampleToNativeGrid(cleaned, rawW, rawH);
  const bbox = findBbox(grid, w, h);
  const outlineColor = detectDarkOutlineColor(grid, w, h);

  // Detect facing direction: side with higher top-half mass (head) is front!
  const midX = (bbox.minX + bbox.maxX) * 0.5;
  const topCutoff = bbox.minY + Math.floor(bbox.bh * 0.42);
  let leftTop = 0, rightTop = 0;
  for (let y = bbox.minY; y <= topCutoff; y++) {
    for (let x = bbox.minX; x <= bbox.maxX; x++) {
      if (grid[(y * w + x) * 4 + 3] > 0) {
        if (x < midX) leftTop++; else rightTop++;
      }
    }
  }
  const facesLeft = leftTop >= rightTop;

  // Locate leg-hip split line (bottom 26%-34% of height where silhouette breaks into leg columns)
  let hipY = bbox.minY + Math.floor(bbox.bh * 0.67);
  let bestRuns = 0;
  for (let y = bbox.minY + Math.floor(bbox.bh * 0.64); y <= bbox.minY + Math.floor(bbox.bh * 0.84); y++) {
    let runs = 0, inRun = false;
    for (let x = bbox.minX; x <= bbox.maxX; x++) {
      const solid = grid[(y * w + x) * 4 + 3] > 0;
      if (solid && !inRun) { runs++; inRun = true; }
      else if (!solid) inRun = false;
    }
    if (runs > bestRuns) {
      bestRuns = runs;
      hipY = Math.max(bbox.minY + Math.floor(bbox.bh * 0.62), y - Math.max(2, Math.floor(bbox.bh * 0.06)));
    }
  }

  // Connected-component clustering of legs below hipY
  const legMap = new Int16Array(w * h).fill(-1);
  const legClusters = [];
  const q = new Int32Array(w * h);
  for (let y = hipY; y <= bbox.maxY; y++) {
    for (let x = bbox.minX; x <= bbox.maxX; x++) {
      const idx = y * w + x;
      if (grid[idx * 4 + 3] === 0 || legMap[idx] !== -1) continue;
      const cid = legClusters.length;
      let head = 0, tail = 0;
      let sx = 0, cnt = 0, minLx = x, maxLx = x, maxLy = y;
      legMap[idx] = cid;
      q[tail++] = idx;
      while (head < tail) {
        const cur = q[head++];
        const cx = cur % w, cy = (cur / w) | 0;
        sx += cx; cnt++;
        if (cx < minLx) minLx = cx;
        if (cx > maxLx) maxLx = cx;
        if (cy > maxLy) maxLy = cy;
        const nbrs = [
          cx > bbox.minX ? cur - 1 : -1,
          cx < bbox.maxX ? cur + 1 : -1,
          cy > hipY ? cur - w : -1,
          cy < bbox.maxY ? cur + w : -1,
        ];
        for (let k = 0; k < 4; k++) {
          const nb = nbrs[k];
          if (nb >= 0 && grid[nb * 4 + 3] > 0 && legMap[nb] === -1) {
            legMap[nb] = cid;
            q[tail++] = nb;
          }
        }
      }
      // Keep clusters that actually reach near the feet
      if (cnt >= 4 && maxLy >= bbox.maxY - Math.max(3, Math.floor(bbox.bh * 0.14))) {
        legClusters.push({ id: cid, cx: sx / cnt, minX: minLx, maxX: maxLx, count: cnt });
      }
    }
  }

  // Decide body archetype: quadruped (4-leg gait), biped (2-leg gait), or blob
  let bodyType = modeHint && modeHint !== "auto" ? modeHint : "quadruped";
  if (!modeHint || modeHint === "auto") {
    const aspect = bbox.bw / Math.max(1, bbox.bh);
    if (legClusters.length >= 3 || (aspect >= 1.05 && legClusters.length >= 2)) bodyType = "quadruped";
    else if (legClusters.length >= 2 || aspect < 1.05) bodyType = "biped";
    else bodyType = "blob";
  }

  // If legs merged into 1-2 wide blobs (e.g. thick front pair & back pair on Polar Bear!),
  // subdivide each wide cluster cleanly into near & far leg columns!
  const partMask = new Uint8Array(w * h); // 0=empty, 1=torso, 2=head, 3=tail, 10=FL, 11=FR, 12=BL, 13=BR
  legClusters.sort((a, b) => a.cx - b.cx);

  // Label every opaque pixel first
  const headXEnd = facesLeft
    ? bbox.minX + Math.floor(bbox.bw * 0.42)
    : bbox.maxX - Math.floor(bbox.bw * 0.42);
  const headYEnd = bbox.minY + Math.floor(bbox.bh * 0.56);
  const tailXStart = facesLeft
    ? bbox.maxX - Math.floor(bbox.bw * 0.18)
    : bbox.minX + Math.floor(bbox.bw * 0.18);

  for (let y = bbox.minY; y <= bbox.maxY; y++) {
    for (let x = bbox.minX; x <= bbox.maxX; x++) {
      const idx = y * w + x;
      if (grid[idx * 4 + 3] === 0) continue;

      if (bodyType !== "blob" && y >= hipY) {
        const relX = (x - bbox.minX) / Math.max(1, bbox.bw);
        if (bodyType === "quadruped") {
          // 4-leg assignment: front pair (0..0.52) split into far/near; back pair (0.52..1.0) split into far/near
          const fx = facesLeft ? relX : (1 - relX);
          if (fx < 0.52) {
            partMask[idx] = fx < 0.27 ? 10 : 11; // Front-A vs Front-B
          } else {
            partMask[idx] = fx < 0.76 ? 12 : 13; // Back-A vs Back-B
          }
        } else {
          // Biped: left leg vs right leg split across center
          partMask[idx] = x < midX ? 10 : 11;
        }
      } else if (y <= headYEnd && (facesLeft ? x <= headXEnd : x >= headXEnd)) {
        partMask[idx] = 2; // Head
      } else if (facesLeft ? x >= tailXStart : x <= tailXStart) {
        partMask[idx] = 3; // Tail / Rear
      } else {
        partMask[idx] = 1; // Torso
      }
    }
  }

  return { grid, w, h, bbox, hipY, facesLeft, bodyType, partMask, outlineColor };
}

function renderRiggedFrame(rig, fx, frameIdx, totalFrames, showDebugRig) {
  const { grid, w, h, bbox, hipY, facesLeft, bodyType, partMask, outlineColor } = rig;
  const out = new Uint8ClampedArray(w * h * 4);
  const debugCol = {
    1: [70, 175, 255],
    2: [255, 200, 75],
    3: [230, 110, 255],
    10: [80, 240, 130],
    11: [255, 115, 115],
    12: [255, 115, 115],
    13: [80, 240, 130],
  };

  const phase = (frameIdx / totalFrames) * Math.PI * 2;
  const dirSign = facesLeft ? -1 : 1;
  const legH = Math.max(4, bbox.maxY - hipY);
  const stridePx = Math.max(2, Math.round(bbox.bw * 0.085));

  // Per-part integer pixel transform (ZERO subpixel blur!)
  let torsoDx = 0, torsoDy = 0, headDx = 0, headDy = 0, tailDx = 0, tailDy = 0;
  // Leg swing: [shearX_at_foot, liftY] for parts 10, 11, 12, 13
  // Diagonal trot pairing: (10 & 13) move together; (11 & 12) move in opposite phase!
  const legAnim = {
    10: { sx: 0, ly: 0 },
    11: { sx: 0, ly: 0 },
    12: { sx: 0, ly: 0 },
    13: { sx: 0, ly: 0 },
  };
  let bodyLeanShear = 0;
  let squashMode = 0; // 0=none, -1=crouch, 1=stretch
  let wholeDy = 0;
  let breatheCompress = 0;

  if (fx === "walk") {
    const sA = Math.sin(phase);
    const sB = Math.sin(phase + Math.PI);
    const liftA = Math.max(0, Math.cos(phase));
    const liftB = Math.max(0, Math.cos(phase + Math.PI));
    const maxLift = Math.max(1, Math.round(legH * 0.22));
    legAnim[10] = { sx: Math.round(sA * stridePx), ly: -Math.round(liftA * maxLift) };
    legAnim[13] = { sx: Math.round(sA * stridePx), ly: -Math.round(liftA * maxLift) };
    legAnim[11] = { sx: Math.round(sB * stridePx), ly: -Math.round(liftB * maxLift) };
    legAnim[12] = { sx: Math.round(sB * stridePx), ly: -Math.round(liftB * maxLift) };
    // Passing bounce (2x frequency): body rises 1px as legs pass under, drops 0px on contact
    torsoDy = -Math.round(Math.abs(Math.cos(phase)) * Math.max(1, Math.round(bbox.bh * 0.03)));
    // Head follow-through lags by 45 degrees (classic Disney overlapping action)
    headDy = -Math.round(Math.abs(Math.cos(phase - 0.8)) * Math.max(1, Math.round(bbox.bh * 0.035)));
    headDx = Math.round(Math.sin(phase - 0.6) * 0.8) * dirSign;
    tailDy = Math.round(Math.sin(phase * 2) * 1.2);
    tailDx = -Math.round(Math.cos(phase) * 0.8) * dirSign;
  } else if (fx === "run") {
    const runStride = Math.max(3, Math.round(bbox.bw * 0.13));
    const maxLift = Math.max(2, Math.round(legH * 0.34));
    const sFront = Math.sin(phase);
    const sBack = Math.sin(phase + Math.PI * 0.85);
    legAnim[10] = { sx: Math.round(sFront * runStride), ly: -Math.round(Math.max(0, Math.cos(phase)) * maxLift) };
    legAnim[11] = { sx: Math.round(Math.sin(phase + 0.5) * runStride), ly: -Math.round(Math.max(0, Math.cos(phase + 0.5)) * maxLift) };
    legAnim[12] = { sx: Math.round(sBack * runStride), ly: -Math.round(Math.max(0, Math.cos(phase + Math.PI * 0.85)) * maxLift) };
    legAnim[13] = { sx: Math.round(Math.sin(phase + Math.PI * 1.15) * runStride), ly: -Math.round(Math.max(0, Math.cos(phase + Math.PI * 1.15)) * maxLift) };
    bodyLeanShear = dirSign * 0.07;
    wholeDy = -Math.round(Math.max(0, Math.sin(phase)) * Math.max(2, Math.round(bbox.bh * 0.06)));
    headDx = dirSign;
    tailDy = Math.round(Math.sin(phase * 2) * 1.5);
  } else if (fx === "breathe") {
    // Classic crisp RPG idle: feet stay 100% locked; upper body & head drop 1px on exhale!
    const exhale = Math.sin(phase) > 0.25 ? 1 : 0;
    breatheCompress = exhale;
    torsoDy = exhale;
    headDy = exhale;
    tailDy = Math.sin(phase + 1.0) > 0.35 ? 1 : 0;
  } else if (fx === "hop") {
    const u = frameIdx / totalFrames;
    if (u < 0.22) {
      squashMode = -1; // Anticipation crouch (feet locked)
      headDy = 2; torsoDy = 1;
    } else if (u < 0.65) {
      const air = Math.sin(((u - 0.22) / 0.43) * Math.PI);
      wholeDy = -Math.round(air * Math.max(4, Math.round(bbox.bh * 0.18)));
      squashMode = air > 0.5 ? 1 : 0;
      legAnim[10] = { sx: dirSign, ly: 1 };
      legAnim[11] = { sx: -dirSign, ly: 1 };
      legAnim[12] = { sx: -dirSign, ly: 1 };
      legAnim[13] = { sx: dirSign, ly: 1 };
    } else if (u < 0.84) {
      squashMode = -1; // Landing squash
      headDy = 1; torsoDy = 1;
    }
  } else if (fx === "wave") {
    // Grounded body, lively tail & head ear ripple
    tailDy = Math.round(Math.sin(phase) * 2.0);
    tailDx = Math.round(Math.cos(phase) * 1.2);
    headDy = Math.round(Math.sin(phase + 1.2) * 0.9);
  }

  // Draw order: Far legs (11, 12) -> Tail (3) -> Torso (1) -> Head (2) -> Near legs (10, 13)
  const drawOrder = [11, 12, 3, 1, 2, 10, 13];
  const putPx = (tx, ty, r, g, b, a) => {
    if (tx < 0 || tx >= w || ty < 0 || ty >= h || a === 0) return;
    const dp = (ty * w + tx) * 4;
    out[dp] = r; out[dp + 1] = g; out[dp + 2] = b; out[dp + 3] = a;
  };

  for (let oi = 0; oi < drawOrder.length; oi++) {
    const pid = drawOrder[oi];
    const isLeg = pid >= 10;
    const la = isLeg ? legAnim[pid] : null;
    for (let y = bbox.minY; y <= bbox.maxY; y++) {
      for (let x = bbox.minX; x <= bbox.maxX; x++) {
        const idx = y * w + x;
        if (partMask[idx] !== pid) continue;
        const sp = idx * 4;
        if (grid[sp + 3] === 0) continue;

        let dx = 0, dy = wholeDy;
        if (isLeg) {
          // Integer Paeth shear around hipY so hip stays attached while paw swings cleanly!
          const tLeg = Math.max(0, Math.min(1, (y - hipY) / Math.max(1, legH)));
          dx = Math.round(la.sx * tLeg);
          dy += Math.round(la.ly * tLeg);
        } else if (pid === 1) {
          dx = torsoDx + Math.round((bbox.maxY - y) * bodyLeanShear);
          dy += torsoDy;
        } else if (pid === 2) {
          dx = headDx + Math.round((bbox.maxY - y) * bodyLeanShear);
          dy += headDy;
        } else if (pid === 3) {
          const distFromBody = facesLeft
            ? Math.max(0, x - (bbox.maxX - bbox.bw * 0.2)) / Math.max(1, bbox.bw * 0.2)
            : Math.max(0, (bbox.minX + bbox.bw * 0.2) - x) / Math.max(1, bbox.bw * 0.2);
          dx = Math.round(tailDx * (0.4 + 0.6 * distFromBody));
          dy += torsoDy + Math.round(tailDy * (0.4 + 0.6 * distFromBody));
        }

        // Squash / stretch on native grid (keeps feet locked at bbox.maxY)
        let ty = y + dy;
        let tx = x + dx;
        if (squashMode === -1 && !isLeg) {
          const fromFoot = bbox.maxY - y;
          ty = bbox.maxY - Math.round(fromFoot * 0.93) + dy;
        } else if (squashMode === 1 && !isLeg) {
          const fromFoot = bbox.maxY - y;
          ty = bbox.maxY - Math.round(fromFoot * 1.06) + dy;
        }

        // Clamp feet never below bottom of canvas during grounded walk
        if (fx === "walk" || fx === "breathe" || fx === "wave") {
          if (ty > bbox.maxY) ty = bbox.maxY;
        }

        let r = grid[sp], g = grid[sp + 1], b = grid[sp + 2];
        if (showDebugRig && debugCol[pid]) {
          r = Math.round(r * 0.55 + debugCol[pid][0] * 0.45);
          g = Math.round(g * 0.55 + debugCol[pid][1] * 0.45);
          b = Math.round(b * 0.55 + debugCol[pid][2] * 0.45);
        }
        putPx(tx, ty, r, g, b, 255);

        // Heal 1px hip socket gap when leg lifts/swings or torso bobs up
        if (isLeg && y <= hipY + 2 && (Math.abs(dx) > 0 || dy < 0 || torsoDy < 0)) {
          putPx(x, y + torsoDy, r, g, b, 255);
        }
      }
    }
  }

  // Pass: Fill single-pixel internal horizontal/vertical seam tears opened between parts
  const snap = new Uint8ClampedArray(out);
  for (let y = 1; y < h - 1; y++) {
    for (let x = 1; x < w - 1; x++) {
      const idx = y * w + x;
      if (snap[idx * 4 + 3] > 0) continue;
      const u = snap[(idx - w) * 4 + 3] > 0;
      const d = snap[(idx + w) * 4 + 3] > 0;
      const l = snap[(idx - 1) * 4 + 3] > 0;
      const r = snap[(idx + 1) * 4 + 3] > 0;
      if ((u && d && (l || r)) || (l && r && (u || d))) {
        const srcIdx = u ? (idx - w) : (idx - 1);
        const sp = srcIdx * 4, dp = idx * 4;
        out[dp] = snap[sp]; out[dp + 1] = snap[sp + 1]; out[dp + 2] = snap[sp + 2]; out[dp + 3] = 255;
      }
    }
  }

  // Optional 1px subtle motion smear trail for Run mode
  if (fx === "run") {
    const trailDx = -dirSign * 2;
    const baseCopy = new Uint8ClampedArray(out);
    for (let y = bbox.minY; y <= bbox.maxY; y++) {
      for (let x = bbox.minX; x <= bbox.maxX; x++) {
        const sp = (y * w + x) * 4;
        if (baseCopy[sp + 3] === 0) continue;
        const tx = x + trailDx;
        if (tx >= 0 && tx < w && out[(y * w + tx) * 4 + 3] === 0 && ((x + y) & 1) === 0) {
          const dp = (y * w + tx) * 4;
          out[dp] = outlineColor[0]; out[dp + 1] = outlineColor[1]; out[dp + 2] = outlineColor[2]; out[dp + 3] = 255;
        }
      }
    }
  }

  return out;
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
  const bodySel = document.getElementById("gif-body-select");
  const modeHint = bodySel ? bodySel.value : "auto";
  gifCustomRig = buildAutoRig(raw, w, h, !cleanChk || cleanChk.checked, modeHint);
  const st = document.getElementById("gif-status");
  if (st) {
    st.textContent = "AUTO-RIG (" + gifCustomRig.bodyType.toUpperCase() + ", " + gifCustomRig.w + "x" + gifCustomRig.h + " GRID)";
  }
}

function syncGifMascotSelection() {
  gifCustomRig = null;
  gifCustomRawImg = null;
  gifCustomName = "";
  const sel = document.getElementById("gif-pet-select");
  const cap = document.getElementById("gif-caption");
  const clr = document.getElementById("gif-clear-btn");
  if (clr) clr.style.display = "none";
  if (sel && cap) {
    const opt = sel.options[sel.selectedIndex];
    cap.value = opt ? opt.textContent.trim() : sel.value;
  }
  const st = document.getElementById("gif-status");
  if (st) st.textContent = "LOSSLESS GIF89A \u00B7 NATIVE PIXEL RIG";
}

function loadGifSpriteFile(file) {
  if (!file) return;
  const cleanName = file.name.replace(/\.[^.]+$/, "").replace(/[^a-zA-Z0-9_-]+/g, "_").slice(0, 24) || "custom_pet";
  const applyLoaded = (img) => {
    gifCustomRawImg = img;
    gifCustomName = cleanName;
    refreshCleanedCustomImg();
    const cap = document.getElementById("gif-caption");
    if (cap) cap.value = cleanName.replace(/_/g, " ").slice(0, 18);
    const clr = document.getElementById("gif-clear-btn");
    if (clr) clr.style.display = "inline-block";
    if (typeof showToast === "function") showToast("Auto-rigged " + file.name + " (" + gifCustomRig.bodyType + ")");
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

function drawNativeRgbaIntegerScaled(targetCtx, rgba, sw, sh, outW, outH, padBottom) {
  const off = document.createElement("canvas");
  off.width = sw; off.height = sh;
  const octx = off.getContext("2d");
  octx.putImageData(new ImageData(rgba, sw, sh), 0, 0);

  // Strict INTEGER scaling so every pixel block is razor-sharp with ZERO mixels!
  const availW = Math.max(sw, outW - 12);
  const availH = Math.max(sh, outH - padBottom - 8);
  const intScale = Math.max(1, Math.floor(Math.min(availW / sw, availH / sh)));
  const dw = sw * intScale;
  const dh = sh * intScale;
  const ox = Math.floor((outW - dw) / 2);
  const oy = Math.max(0, outH - padBottom - dh);
  targetCtx.imageSmoothingEnabled = false;
  targetCtx.drawImage(off, 0, 0, sw, sh, ox, oy, dw, dh);
}

function drawGifMakerFrame(targetCtx, w, h, frameIdx, totalFrames, allowDebug) {
  const petId = document.getElementById("gif-pet-select").value;
  const fx = document.getElementById("gif-fx-select").value;
  const bgMode = document.getElementById("gif-bg-select").value;
  const caption = (document.getElementById("gif-caption").value || "").trim();
  const dbgEl = document.getElementById("gif-show-rig");
  const showRig = allowDebug && dbgEl && dbgEl.checked;
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

  const padBottom = bgMode === "card" ? Math.round(10 * amp) : Math.round(4 * amp);
  if (gifCustomRig) {
    const frameRgba = renderRiggedFrame(gifCustomRig, fx, frameIdx, totalFrames, showRig);
    drawNativeRgbaIntegerScaled(targetCtx, frameRgba, gifCustomRig.w, gifCustomRig.h, w, h, padBottom);
    return;
  }

  // Built-in 9-column crisp sprite sheets (uses authored walk frames directly when fx === "walk"!)
  if (typeof sprites !== "undefined" && sprites[petId] && sprites[petId].complete && sprites[petId].naturalWidth > 0) {
    const sp = sprites[petId];
    const fw = Math.floor(sp.naturalWidth / 9);
    const fh = sp.naturalHeight;
    const fCol = (fx === "walk" || fx === "run") ? (1 + (frameIdx % 4)) : 0;
    const dy = fx === "hop" ? -Math.round(Math.sin((frameIdx / totalFrames) * Math.PI) * 14 * amp) : 0;
    const box = w - Math.round((bgMode === "card" ? 28 : 12) * amp);
    const ox = Math.floor((w - box) / 2);
    const oy = h - padBottom - box + dy;
    targetCtx.drawImage(sp, fCol * fw, 0, fw, fh, ox, oy, box, box);
  }
}

// GIF89a LZW encoder (exact decoder-synced bit width increment: nextCode > (1 << codeSize))
function lzwEncodeGif(indices, minCodeSize) {
  const clearCode = 1 << minCodeSize;
  const eoiCode = clearCode + 1;
  let codeSize = minCodeSize + 1;
  let nextCode = eoiCode + 1;
  let dict = new Map();
  const out = [];
  let bitBuf = 0, bitCount = 0;

  function emit(code) {
    bitBuf |= (code << bitCount);
    bitCount += codeSize;
    while (bitCount >= 8) {
      out.push(bitBuf & 0xff);
      bitBuf >>= 8;
      bitCount -= 8;
    }
  }

  emit(clearCode);
  if (indices.length === 0) {
    emit(eoiCode);
    if (bitCount > 0) out.push(bitBuf & 0xff);
    return out;
  }

  let prefix = indices[0];
  for (let i = 1; i < indices.length; i++) {
    const k = indices[i];
    const key = (prefix << 8) | k;
    const hit = dict.get(key);
    if (hit !== undefined) {
      prefix = hit;
    } else {
      emit(prefix);
      if (nextCode < 4096) {
        dict.set(key, nextCode);
        nextCode++;
        if (nextCode > (1 << codeSize) && codeSize < 12) codeSize++;
      } else {
        emit(clearCode);
        dict.clear();
        codeSize = minCodeSize + 1;
        nextCode = eoiCode + 1;
      }
      prefix = k;
    }
  }
  emit(prefix);
  emit(eoiCode);
  if (bitCount > 0) out.push(bitBuf & 0xff);
  return out;
}

function buildGifPalette(framesRgba, transparent) {
  const uniqueMap = new Map();
  const counts = [];
  for (let f = 0; f < framesRgba.length; f++) {
    const rgba = framesRgba[f];
    for (let p = 0; p < rgba.length; p += 4) {
      if (transparent && rgba[p + 3] < 128) continue;
      const r = rgba[p], g = rgba[p + 1], b = rgba[p + 2];
      const key = (r << 16) | (g << 8) | b;
      const idx = uniqueMap.get(key);
      if (idx === undefined) {
        uniqueMap.set(key, counts.length);
        counts.push([r, g, b, 1]);
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
      const rR = maxR - minR, rG = maxG - minG, rB = maxB - minB;
      const m = Math.max(rR, rG, rB);
      if (m > bestRange) {
        bestRange = m; splitIdx = i;
        splitChan = m === rG ? 1 : (m === rR ? 0 : 2);
      }
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
    for (let j = 0; j < box.length; j++) {
      const wt = box[j][3];
      sr += box[j][0] * wt; sg += box[j][1] * wt; sb += box[j][2] * wt; sw += wt;
    }
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
    const rgba = framesRgba[f];
    const idxBuf = new Uint8Array(w * h);
    for (let p = 0; p < w * h; p++) {
      if (transparent && rgba[p * 4 + 3] < 128) { idxBuf[p] = 0; continue; }
      const r = rgba[p * 4], g = rgba[p * 4 + 1], b = rgba[p * 4 + 2];
      const key = (r << 16) | (g << 8) | b;
      let ci = lookup.get(key);
      if (ci === undefined) {
        let best = 1, bestD = 1e9;
        for (let j = 1; j < palette.length; j++) {
          const dr = r - palette[j][0], dg = g - palette[j][1], db = b - palette[j][2];
          const d = dr * dr * 2 + dg * dg * 4 + db * db * 3;
          if (d < bestD) { bestD = d; best = j; }
        }
        ci = best;
        lookup.set(key, ci);
      }
      idxBuf[p] = ci;
    }
    indexedFrames.push(idxBuf);
  }

  const bytes = [];
  const pushStr = (s) => { for (let i = 0; i < s.length; i++) bytes.push(s.charCodeAt(i)); };
  const pushU16 = (v) => { bytes.push(v & 0xff, (v >> 8) & 0xff); };

  pushStr("GIF89a");
  pushU16(w); pushU16(h);
  bytes.push(0xF7, 0x00, 0x00);
  for (let i = 0; i < 256; i++) {
    const c = palette[i] || [0, 0, 0];
    bytes.push(c[0], c[1], c[2]);
  }
  bytes.push(0x21, 0xFF, 0x0B);
  pushStr("NETSCAPE2.0");
  bytes.push(0x03, 0x01, 0x00, 0x00, 0x00);

  for (let f = 0; f < indexedFrames.length; f++) {
    bytes.push(0x21, 0xF9, 0x04, transparent ? 0x09 : 0x08);
    pushU16(delayCs);
    bytes.push(0x00, 0x00);
    bytes.push(0x2C);
    pushU16(0); pushU16(0); pushU16(w); pushU16(h);
    bytes.push(0x00, 8);
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

    if (!gifCustomRig && (!sprites[petId] || !sprites[petId].complete || sprites[petId].naturalWidth === 0)) {
      statusEl.textContent = "WAIT: SPRITE STILL LOADING...";
      return;
    }

    const totalFrames = getGifTotalFrames(fx);
    const frames = [];
    for (let i = 0; i < totalFrames; i++) {
      drawGifMakerFrame(octx, size, size, i, totalFrames, false);
      frames.push(octx.getImageData(0, 0, size, size).data);
    }
    const delayCs = Math.max(4, Math.round(100 / fps));
    const gifBytes = encodeGif89a(frames, size, size, delayCs, transparent);
    const blob = new Blob([gifBytes], { type: "image/gif" });
    const slug = (gifCustomRig && gifCustomName) ? gifCustomName : petId;
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = slug + "-" + fx + "-" + size + "px.gif";
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 2500);
    statusEl.textContent = "EXPORTED " + (gifBytes.length / 1024).toFixed(1) + " KB (" + size + "px, " + totalFrames + "f)";
    if (typeof showToast === "function") showToast("Exported " + a.download);
  } catch (err) {
    console.error("GIF export error:", err);
    statusEl.textContent = "ERROR: USE HTTPS OR UPLOAD A LOCAL FILE";
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
  syncGifMascotSelection();
  updateGifPreviewFps();
}

if (typeof module !== "undefined" && module.exports) {
  module.exports = { cleanRgbaPixels, buildAutoRig, renderRiggedFrame, encodeGif89a };
}
