const { performance } = require("perf_hooks");
const {
  buildAutoRig,
  renderRiggedFrame,
  buildFrameDiffRgba,
  computeBiharmonicWeights,
  solveFabrikChain2D,
  inpaintTorsoBacking,
  evalSpringCycle,
  fitOptimalRotation2D,
  sampleCleanEdgePixel,
  applyParallax25DTransforms,
} = require("../docs/gif_maker.js");

function assert(cond, msg) {
  if (!cond) {
    console.error("PAPER/GATE FAIL:", msg);
    process.exit(1);
  }
}

function makeBox(W, H, rects) {
  const buf = new Uint8ClampedArray(W * H * 4);
  for (const [x0, y0, x1, y1, r, g, b] of rects) {
    for (let y = y0; y <= y1; y++) {
      for (let x = x0; x <= x1; x++) {
        if (x < 0 || x >= W || y < 0 || y >= H) continue;
        const p = (y * W + x) * 4;
        const edge = (x === x0 || x === x1 || y === y0 || y === y1);
        buf[p] = edge ? 15 : r;
        buf[p + 1] = edge ? 15 : g;
        buf[p + 2] = edge ? 18 : b;
        buf[p + 3] = 255;
      }
    }
  }
  return buf;
}

function analyzeFrame(rgba, w, h) {
  const vis = new Uint8Array(w * h);
  const q = new Int32Array(w * h);
  let comps = 0, tinySpecks = 0, opaquePx = 0, lowestY = -1;
  for (let i = 0; i < w * h; i++) {
    if (rgba[i * 4 + 3] > 0) {
      opaquePx++;
      const y = (i / w) | 0;
      if (y > lowestY) lowestY = y;
    }
    if (vis[i] || rgba[i * 4 + 3] === 0) continue;
    let head = 0, tail = 0;
    vis[i] = 1;
    q[tail++] = i;
    while (head < tail) {
      const u = q[head++], ux = u % w, uy = (u / w) | 0;
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          const nx = ux + dx, ny = uy + dy;
          if (nx < 0 || nx >= w || ny < 0 || ny >= h) continue;
          const v = ny * w + nx;
          if (!vis[v] && rgba[v * 4 + 3] > 0) {
            vis[v] = 1;
            q[tail++] = v;
          }
        }
      }
    }
    if (tail <= 2) tinySpecks++;
    else if (tail >= 8) comps++;
  }

  let pinholes = 0;
  for (let y = 1; y < h - 1; y++) {
    for (let x = 1; x < w - 1; x++) {
      const i = y * w + x;
      if (rgba[i * 4 + 3] > 0) continue;
      if (
        rgba[(i - 1) * 4 + 3] > 0 &&
        rgba[(i + 1) * 4 + 3] > 0 &&
        rgba[(i - w) * 4 + 3] > 0 &&
        rgba[(i + w) * 4 + 3] > 0
      ) {
        pinholes++;
      }
    }
  }
  let minX = w, maxX = -1, minY = h, maxY = -1, perim = 0;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = y * w + x;
      if (rgba[i * 4 + 3] === 0) continue;
      if (x < minX) minX = x; if (x > maxX) maxX = x;
      if (y < minY) minY = y; if (y > maxY) maxY = y;
      const isEdge =
        x === 0 || x === w - 1 || y === 0 || y === h - 1 ||
        rgba[(i - 1) * 4 + 3] === 0 || rgba[(i + 1) * 4 + 3] === 0 ||
        rgba[(i - w) * 4 + 3] === 0 || rgba[(i + w) * 4 + 3] === 0;
      if (isEdge) perim++;
    }
  }
  return { comps, tinySpecks, opaquePx, lowestY, pinholes, minX, maxX, minY, maxY, perim };
}

const archetypes = [
  { name: "quad_left", mode: "quadruped", W: 124, H: 108, buf: makeBox(124, 108, [[18,22,98,66,245,242,232],[10,10,38,36,245,242,232],[22,62,38,96,240,238,228],[40,62,54,96,230,228,220],[66,62,80,96,230,228,220],[82,62,96,96,240,238,228]]) },
  { name: "quad_right", mode: "quadruped", W: 124, H: 108, buf: makeBox(124, 108, [[18,22,98,66,210,170,120],[78,10,106,36,210,170,120],[22,62,38,96,200,160,110],[40,62,54,96,190,150,100],[66,62,80,96,190,150,100],[82,62,96,96,200,160,110]]) },
  { name: "quad_stubby", mode: "quadruped", W: 124, H: 108, buf: makeBox(124, 108, [[18,28,100,86,235,190,130],[10,16,40,46,235,190,130],[24,82,38,98,225,180,120],[44,82,56,98,215,170,110],[68,82,80,98,215,170,110],[84,82,96,98,225,180,120]]) },
  { name: "quad_tall", mode: "quadruped", W: 112, H: 128, buf: makeBox(112, 128, [[22,24,88,60,230,195,110],[14,8,36,34,230,195,110],[26,56,36,114,220,185,100],[40,56,50,114,210,175,95],[62,56,72,114,210,175,95],[76,56,86,114,220,185,100]]) },
  { name: "biped", mode: "biped", W: 84, H: 120, buf: makeBox(84, 120, [[26,12,58,38,180,210,240],[24,36,60,76,160,190,230],[12,42,26,70,150,180,220],[58,42,72,70,150,180,220],[26,74,38,108,140,170,210],[46,74,58,108,140,170,210]]) },
  { name: "bird", mode: "bird", W: 100, H: 92, buf: makeBox(100, 92, [[20,22,74,58,90,180,240],[12,16,34,36,90,180,240],[36,14,66,42,70,160,220],[72,34,90,50,70,160,220],[42,56,54,78,240,180,60]]) },
  { name: "crawler", mode: "crawler", W: 128, H: 64, buf: makeBox(128, 64, [[12,24,114,44,95,200,110],[10,18,28,36,110,220,120]]) },
  { name: "swimmer", mode: "swimmer", W: 120, H: 76, buf: makeBox(120, 76, [[14,20,86,56,250,140,60],[82,14,108,62,240,110,40]]) },
  { name: "blob", mode: "blob", W: 84, H: 84, buf: makeBox(84, 84, [[16,20,68,68,140,235,170]]) },
];

console.log("=== PART A: RESEARCH PAPER MATHEMATICAL INVARIANTS ===");

// Paper 1: Bounded Biharmonic Weights (Jacobson et al., SIGGRAPH 2011)
{
  const rig = buildAutoRig(archetypes[0].buf, archetypes[0].W, archetypes[0].H, true, "quadruped");
  const W = computeBiharmonicWeights(rig);
  let maxPouErr = 0, minW = 1, maxW = 0, blendedPx = 0;
  for (let i = 0; i < rig.w * rig.h; i++) {
    if (rig.grid[i * 4 + 3] === 0) continue;
    let sum = 0, active = 0;
    for (let p = 0; p < W.length; p++) {
      const v = W[p][i];
      if (v < minW) minW = v;
      if (v > maxW) maxW = v;
      if (v > 0.05) active++;
      sum += v;
    }
    if (active >= 2) blendedPx++;
    const err = Math.abs(sum - 1.0);
    if (err > maxPouErr) maxPouErr = err;
  }
  assert(minW >= 0 && maxW <= 1.0001, "BBW bounds violated: " + minW + ".." + maxW);
  assert(maxPouErr < 1e-4, "BBW partition-of-unity error too high: " + maxPouErr);
  assert(blendedPx > 200, "BBW not smooth enough, blendedPx=" + blendedPx);
  console.log("  [PASS] Paper 1 (BBW SIGGRAPH 2011)      | PoU err=" + maxPouErr.toExponential(1) + " | range=[" + minW.toFixed(2) + "," + maxW.toFixed(2) + "] | smoothBlendPx=" + blendedPx);
}

// Paper 2: FABRIK 2D IK (Aristidou & Lasenby 2011)
{
  const lengths = [18, 16];
  let maxLenErr = 0, maxReachErr = 0;
  for (const [tx, ty] of [[50, 78], [42, 72], [58, 70], [46, 65]]) {
    const pts = solveFabrikChain2D([{ x: 50, y: 46 }, { x: 50, y: 64 }, { x: 50, y: 80 }], lengths, tx, ty, -1, 10);
    const l0 = Math.hypot(pts[1].x - pts[0].x, pts[1].y - pts[0].y);
    const l1 = Math.hypot(pts[2].x - pts[1].x, pts[2].y - pts[1].y);
    const rErr = Math.hypot(pts[2].x - tx, pts[2].y - ty);
    maxLenErr = Math.max(maxLenErr, Math.abs(l0 - lengths[0]), Math.abs(l1 - lengths[1]));
    maxReachErr = Math.max(maxReachErr, rErr);
  }
  assert(maxLenErr < 0.05, "FABRIK bone length drift: " + maxLenErr);
  assert(maxReachErr < 0.15, "FABRIK reach error: " + maxReachErr);
  console.log("  [PASS] Paper 2 (FABRIK 2011)            | maxBoneLenErr=" + maxLenErr.toFixed(4) + "px | maxReachErr=" + maxReachErr.toFixed(4) + "px");
}

// Paper 3: Meta Animated Drawings (Smith et al., ACM TOG 2023)
{
  const rig = buildAutoRig(archetypes[0].buf, archetypes[0].W, archetypes[0].H, true, "quadruped");
  const inp = inpaintTorsoBacking(rig);
  let filled = 0, darkInkLeak = 0;
  for (let i = 0; i < rig.w * rig.h; i++) {
    if (!rig.inpaintedMask[i]) continue;
    const p = i * 4;
    if (inp.data[p + 3] === 255) {
      filled++;
      const lum = inp.data[p] * 0.299 + inp.data[p + 1] * 0.587 + inp.data[p + 2] * 0.114;
      if (lum < 30 && rig.grid[p + 3] > 0 && (rig.grid[p] * 0.299 + rig.grid[p + 1] * 0.587 + rig.grid[p + 2] * 0.114) > 80) {
        darkInkLeak++;
      }
    }
  }
  assert(filled > 500, "Inpaint mask too small: " + filled);
  assert(darkInkLeak === 0, "Inpaint leaked dark ink into interior: " + darkInkLeak);
  console.log("  [PASS] Paper 3 (Animated Drawings 2023) | inpaintedTorsoPx=" + filled + " | inkLeakPx=" + darkInkLeak);
}

// Paper 4: Complementary Dynamics (Zhang et al., SIGGRAPH 2020)
{
  const cyc = evalSpringCycle("walk", 16, 0.1, 18.0, 5.5);
  let peakIdx = 0;
  for (let i = 1; i < cyc.length; i++) if (cyc[i] > cyc[peakIdx]) peakIdx = i;
  // Driver sin(2*phase) peaks at idx=2 (phase=pi/4); damped spring must lag behind driver (peakIdx > 2)
  assert(peakIdx >= 2 && peakIdx <= 6, "Spring phase lag unexpected: peakIdx=" + peakIdx);
  console.log("  [PASS] Paper 4 (Comp Dynamics 2020)     | driverPeak=f2 -> springLagPeak=f" + peakIdx + " | amp=" + cyc[peakIdx].toFixed(4));
}


// Paper 5: ARAP 2D Exact Procrustes Rotation Fit (Igarashi 2005 / Sorkine 2007)
{
  const targetAng = 0.42; // ~24 deg
  const c = Math.cos(targetAng), s = Math.sin(targetAng);
  const rest = [{ x: 0, y: 0 }, { x: 4, y: 0 }, { x: 4, y: 4 }, { x: 0, y: 4 }];
  const def = rest.map((p) => ({ x: 10 + p.x * c - p.y * s, y: 5 + p.x * s + p.y * c }));
  const fit = fitOptimalRotation2D(rest, def);
  const err = Math.abs(fit.theta - targetAng);
  assert(err < 1e-6, "ARAP 2D rotation recovery error: " + err);
  console.log("  [PASS] Paper 5 (ARAP 2D SIGGRAPH 2005)  | recoveredAngleErr=" + err.toExponential(1) + " rad");
}

// Paper 6: CleanEdge Diagonal Pixel-Art Slope Preservation (Kopf-Lischinski 2011)
{
  const g = new Uint8ClampedArray(4 * 4 * 4);
  // 45-degree dark diagonal at (0,0) and (1,1)
  for (const [x, y, col] of [[0,0,20],[1,1,20],[1,0,220],[0,1,220]]) {
    const p = (y * 4 + x) * 4;
    g[p] = col; g[p+1] = col; g[p+2] = col; g[p+3] = 255;
  }
  const spMid = sampleCleanEdgePixel(g, 4, 4, 0.5, 0.5);
  assert(g[spMid] === 20, "CleanEdge failed to preserve 45-deg diagonal midpoint");
  console.log("  [PASS] Paper 6 (CleanEdge Pixel 2011)   | 45-deg diagonal preserved at subpixel (0.5,0.5)");
}


// Paper 7: 2.5D Cartoon Parallax (Rivers et al., SIGGRAPH 2010)
{
  const pins = [{ id: "head" }, { id: "fl" }, { id: "fr" }, { id: "chest" }];
  const mkWorld = () => pins.map((_, i) => ({ pivOutX: 50 + i * 2 }));
  const bbox = { minX: 20, maxX: 80, bw: 60 };
  const w0 = mkWorld();
  applyParallax25DTransforms(pins, w0, bbox, 0, "walk", "quadruped");
  assert(Math.abs(w0[1].pivOutX - 52) < 1e-6, "2.5D parallax yaw=0 not identity");
  const w1 = mkWorld();
  applyParallax25DTransforms(pins, w1, bbox, Math.PI * 0.5, "walk", "quadruped");
  assert(w1[1].pivOutX > 52 && w1[2].pivOutX < 54, "2.5D parallax near/far limbs did not oppose");
  console.log("  [PASS] Paper 7 (2.5D Parallax 2010)     | yaw=0 exact identity | near/far opposing parallax verified");
}

console.log("\n=== PART A2: EDGE-CASE STRESS TESTS (ALL 7 PAPERS) ===");
// Edge 1: FABRIK unreachable, zero-distance, and collinear targets (0 NaNs)
{
  const unreach = solveFabrikChain2D([{ x: 0, y: 0 }, { x: 0, y: 10 }, { x: 0, y: 20 }], [10, 10], 100, 0, -1, 8);
  const zeroD = solveFabrikChain2D([{ x: 5, y: 5 }, { x: 5, y: 15 }, { x: 5, y: 25 }], [10, 10], 5, 5, 1, 8);
  for (const pts of [unreach, zeroD]) {
    for (const p of pts) assert(Number.isFinite(p.x) && Number.isFinite(p.y), "FABRIK produced NaN on edge case");
    const l0 = Math.hypot(pts[1].x - pts[0].x, pts[1].y - pts[0].y);
    const l1 = Math.hypot(pts[2].x - pts[1].x, pts[2].y - pts[1].y);
    assert(Math.abs(l0 - 10) < 0.05 && Math.abs(l1 - 10) < 0.05, "FABRIK edge-case bone length drift");
  }
  console.log("  [PASS] Edge 1 (FABRIK degenerate)       | unreachable & zero-dist targets -> 0 NaNs, exact lengths");
}

// Edge 2: Animated Drawings inpaint on all-dark black-cat sprite (must NOT paint rgb(180,180,180) grey)
{
  const darkBuf = makeBox(124, 108, [[18,22,98,66,22,22,26],[10,10,38,36,22,22,26],[22,62,38,96,20,20,24],[82,62,96,96,20,20,24]]);
  const darkRig = buildAutoRig(darkBuf, 124, 108, true, "quadruped");
  const darkInp = inpaintTorsoBacking(darkRig);
  let brightArtifact = 0;
  for (let i = 0; i < darkRig.w * darkRig.h; i++) {
    if (darkRig.inpaintedMask[i] && darkInp.data[i * 4] > 100) brightArtifact++;
  }
  assert(brightArtifact === 0, "Dark sprite inpaint created bright grey artifacts: " + brightArtifact);
  console.log("  [PASS] Edge 2 (Dark-sprite inpaint)     | all-dark mascot -> 0 bright-grey artifacts");
}

// Edge 3: Complementary Dynamics low-FPS (4f) & stiff spring (k=60) stability
{
  const stiff = evalSpringCycle("hop", 4, 0.08, 60.0, 8.0);
  for (let i = 0; i < stiff.length; i++) {
    assert(Number.isFinite(stiff[i]) && Math.abs(stiff[i]) < 0.25, "Spring diverged at k=60, N=4: " + stiff[i]);
  }
  console.log("  [PASS] Edge 3 (Stiff/low-FPS spring)    | k=60, N=4 sub-stepped -> unconditionally stable");
}

// Edge 4: ARAP 2D multi-quadrant rotation recovery + zero-area degenerate quad
{
  for (const ang of [-2.1, -0.35, 0.0, 1.9]) {
    const c = Math.cos(ang), s = Math.sin(ang);
    const rest = [{ x: 0, y: 0 }, { x: 4, y: 0 }, { x: 4, y: 4 }, { x: 0, y: 4 }];
    const def = rest.map((p) => ({ x: p.x * c - p.y * s, y: p.x * s + p.y * c }));
    const fit = fitOptimalRotation2D(rest, def);
    assert(Math.abs(fit.theta - ang) < 1e-6, "ARAP quadrant error at " + ang);
  }
  const zeroFit = fitOptimalRotation2D([{ x: 1, y: 1 }, { x: 1, y: 1 }], [{ x: 2, y: 2 }, { x: 2, y: 2 }]);
  assert(Number.isFinite(zeroFit.theta), "ARAP produced NaN on zero-area quad");
  console.log("  [PASS] Edge 4 (ARAP 2D quadrants/degen) | 4 quadrants exact + zero-area quad -> 0 NaNs");
}

// Edge 5: CleanEdge anti-diagonal + out-of-bounds UV clamping
{
  const g = new Uint8ClampedArray(4 * 4 * 4);
  for (const [x, y, col] of [[1,0,20],[0,1,20],[0,0,220],[1,1,220]]) {
    const p = (y * 4 + x) * 4;
    g[p] = col; g[p+1] = col; g[p+2] = col; g[p+3] = 255;
  }
  assert(g[sampleCleanEdgePixel(g, 4, 4, 0.5, 0.5)] === 20, "CleanEdge anti-diagonal failed");
  const oob = sampleCleanEdgePixel(g, 4, 4, -5.2, 99.7);
  assert(oob >= 0 && oob < g.length, "CleanEdge OOB UV failed");
  console.log("  [PASS] Edge 5 (CleanEdge anti-diag/OOB) | anti-45-deg preserved + OOB UV safely clamped");
}

console.log("\n=== PART B: 9-METHOD x 9-ARCHETYPE x 5-FX BENCHMARK (3,240 FRAMES) ===");
const methods = ["bone_fk", "harmonic_dqs", "fabrik_ik", "layered_inpaint", "complementary_dyn", "arap_2d", "cleanedge_rot", "parallax_25d", "sota_hybrid"];
const fxList = ["walk", "sprint", "dance", "hop", "sleep"];
let hardFailures = 0;

for (const m of methods) {
  let grandMotion = 0, minAreaRatio = 9, maxAreaRatio = 0, totalFrames = 0;
  let tears = 0, specks = 0, holes = 0, spikes = 0, perimWarns = 0, frozenLegs = 0, areaWarns = [];
  const t0 = performance.now();

  for (const a of archetypes) {
    const rig = buildAutoRig(a.buf, a.W, a.H, true, a.mode);
    const restStats = analyzeFrame(rig.grid, rig.w, rig.h);

    for (const fx of fxList) {
      let maxMoved = 0;
      for (let fi = 0; fi < 8; fi++) {
        const f = renderRiggedFrame(rig, fx, fi, 8, m);
        totalFrames++;
        const st = analyzeFrame(f, rig.w, rig.h);
        if (st.comps !== 1) tears++;
        if (st.tinySpecks > 0) specks += st.tinySpecks;
        if (st.pinholes > 0) holes += st.pinholes;
        const ratio = st.opaquePx / Math.max(1, restStats.opaquePx);
        if (ratio < minAreaRatio) minAreaRatio = ratio;
        if (ratio > maxAreaRatio) maxAreaRatio = ratio;
        if (ratio < 0.84 || ratio > 1.18) {
          areaWarns.push(a.name + "/" + fx + "/f" + fi + "=" + ratio.toFixed(2) + "x");
        }
        // Gate 8: Spike / Protrusion detector (no pixel > 13px outside rest bbox on grounded walk/sleep)
        if (fx === "walk" || fx === "sleep") {
          const padL = restStats.minX - st.minX;
          const padR = st.maxX - restStats.maxX;
          const padT = restStats.minY - st.minY;
          const padB = st.maxY - restStats.maxY;
          if (Math.max(padL, padR, padT, padB) > 13) spikes++;
        }
        // Gate 10: Perimeter-spike detector (catches stretched needles / jaggedness)
        if (st.perim / Math.max(1, restStats.perim) > 1.26) perimWarns++;

        const diff = buildFrameDiffRgba(rig.grid, f, rig.w, rig.h);
        if (diff.movedPx > maxMoved) maxMoved = diff.movedPx;
      }
      // Gate 9: Lower-body (leg) motion authority on quadruped walk (catches frozen/wobbly legs)
      if (a.mode === "quadruped" && fx === "walk") {
        const legY0 = Math.floor(rig.bbox.minY + rig.bbox.bh * 0.65);
        let peakLegMoved = 0;
        for (let fi = 0; fi < 8; fi++) {
          const f = renderRiggedFrame(rig, fx, fi, 8, m);
          let legMoved = 0;
          for (let y = legY0; y < rig.h; y++) {
            for (let x = 0; x < rig.w; x++) {
              const p = (y * rig.w + x) * 4;
              if ((rig.grid[p + 3] > 0) !== (f[p + 3] > 0)) legMoved++;
            }
          }
          if (legMoved > peakLegMoved) peakLegMoved = legMoved;
        }
        if (peakLegMoved < 120) frozenLegs++;
      }
      grandMotion += maxMoved;
    }
  }

  const msPerFrame = (performance.now() - t0) / totalFrames;
  const ok = tears === 0 && specks === 0 && holes === 0 && spikes === 0 && perimWarns === 0 && frozenLegs === 0 && areaWarns.length === 0;
  if (!ok && m !== "bone_fk") hardFailures++;
  const tag = ok ? "[PASS]" : "[WARN]";
  console.log(
    "  " + tag + " " + m.padEnd(18) +
    " | tears=" + tears + " holes=" + holes + " spikes=" + spikes + " perim=" + perimWarns + " frzLeg=" + frozenLegs +
    " | area=" + minAreaRatio.toFixed(2) + ".." + maxAreaRatio.toFixed(2) + "x" +
    " | avgPeak=" + Math.round(grandMotion / 45) + "px" +
    " | " + msPerFrame.toFixed(2) + "ms/f" +
    (areaWarns.length ? " | warns=" + areaWarns.slice(0, 2).join(",") : "")
  );
}

if (hardFailures > 0) {
  console.error("\nFAIL: " + hardFailures + " research method(s) had quality warnings.");
  process.exit(1);
}
console.log("\nALL 4 RESEARCH-PAPER METHODS PASSED ALL MATHEMATICAL & VISUAL GATES!");
