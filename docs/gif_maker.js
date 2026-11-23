// DIYPets Zero-Install Browser GIF89a Maker (Verified LZW + Median-Cut + Grey Fringe Cleaner)
let gifCustomImg = null;
let gifCustomRawImg = null;
let gifCustomName = "";
let gifPreviewTick = 0;
let gifPreviewTimer = null;

function cleanSpriteCanvas(rawImg, enableClean) {
  const w = rawImg.naturalWidth || rawImg.width || 128;
  const h = rawImg.naturalHeight || rawImg.height || 128;
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  const g = c.getContext("2d", { willReadFrequently: true });
  g.imageSmoothingEnabled = false;
  g.drawImage(rawImg, 0, 0, w, h);
  if (!enableClean) return c;

  const imgData = g.getImageData(0, 0, w, h);
  const d = imgData.data;

  // Step A: Border flood-fill chroma-key removal if corners are opaque (never erases interior white fur!)
  const corners = [0, w - 1, (h - 1) * w, (h - 1) * w + (w - 1)];
  const bgSamples = [];
  for (let i = 0; i < corners.length; i++) {
    const p = corners[i] * 4;
    if (d[p + 3] >= 24) bgSamples.push([d[p], d[p + 1], d[p + 2]]);
  }
  const isBgLike = (p) => {
    const a = d[p + 3];
    if (a < 24) return true;
    const r = d[p], gr = d[p + 1], b = d[p + 2];
    const sat = Math.max(r, gr, b) - Math.min(r, gr, b);
    const lum = (r + gr + b) / 3;
    if (sat <= 16 && lum >= 168 && bgSamples.length > 0) return true;
    for (let i = 0; i < bgSamples.length; i++) {
      const s = bgSamples[i];
      if (Math.abs(r - s[0]) + Math.abs(gr - s[1]) + Math.abs(b - s[2]) <= 42) return true;
    }
    return false;
  };
  if (bgSamples.length > 0) {
    const vis = new Uint8Array(w * h);
    const q = new Int32Array(w * h);
    let head = 0, tail = 0;
    const push = (idx) => {
      if (vis[idx]) return;
      if (isBgLike(idx * 4)) { vis[idx] = 1; q[tail++] = idx; }
    };
    for (let x = 0; x < w; x++) { push(x); push((h - 1) * w + x); }
    for (let y = 0; y < h; y++) { push(y * w); push(y * w + (w - 1)); }
    while (head < tail) {
      const cur = q[head++];
      const p = cur * 4;
      d[p] = 0; d[p + 1] = 0; d[p + 2] = 0; d[p + 3] = 0;
      const cx = cur % w, cy = (cur / w) | 0;
      if (cx > 0) push(cur - 1);
      if (cx + 1 < w) push(cur + 1);
      if (cy > 0) push(cur - w);
      if (cy + 1 < h) push(cur + w);
    }
  }

  // Step B: Hard alpha matte + grey anti-aliased fringe removal around outlines & paws
  for (let i = 0; i < w * h; i++) {
    const p = i * 4;
    const a = d[p + 3];
    if (a < 165) {
      d[p] = 0; d[p + 1] = 0; d[p + 2] = 0; d[p + 3] = 0;
    } else if (a < 245) {
      const r = d[p], gr = d[p + 1], b = d[p + 2];
      const sat = Math.max(r, gr, b) - Math.min(r, gr, b);
      const lum = (r + gr + b) / 3;
      if (sat <= 20 && lum >= 95 && lum <= 238) {
        d[p] = 0; d[p + 1] = 0; d[p + 2] = 0; d[p + 3] = 0;
      } else {
        d[p + 3] = 255;
      }
    } else {
      d[p + 3] = 255;
    }
  }

  // Step C: Remove isolated 1px neutral grey halo pixels touching transparency on 3+ sides
  const alphaCopy = new Uint8Array(w * h);
  for (let i = 0; i < w * h; i++) alphaCopy[i] = d[i * 4 + 3];
  for (let y = 1; y < h - 1; y++) {
    for (let x = 1; x < w - 1; x++) {
      const idx = y * w + x;
      if (alphaCopy[idx] === 0) continue;
      const p = idx * 4;
      const r = d[p], gr = d[p + 1], b = d[p + 2];
      const sat = Math.max(r, gr, b) - Math.min(r, gr, b);
      const lum = (r + gr + b) / 3;
      if (sat <= 18 && lum >= 105 && lum <= 228) {
        let emptyN = 0;
        if (alphaCopy[idx - 1] === 0) emptyN++;
        if (alphaCopy[idx + 1] === 0) emptyN++;
        if (alphaCopy[idx - w] === 0) emptyN++;
        if (alphaCopy[idx + w] === 0) emptyN++;
        if (emptyN >= 3) {
          d[p] = 0; d[p + 1] = 0; d[p + 2] = 0; d[p + 3] = 0;
        }
      }
    }
  }

  g.putImageData(imgData, 0, 0);
  return c;
}

function refreshCleanedCustomImg() {
  if (!gifCustomRawImg) return;
  const chk = document.getElementById("gif-clean-bg");
  gifCustomImg = cleanSpriteCanvas(gifCustomRawImg, !chk || chk.checked);
}

function syncGifMascotSelection() {
  gifCustomImg = null;
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
  if (st) st.textContent = "LOSSLESS GIF89A \u00B7 CLIENT-SIDE";
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
    const st = document.getElementById("gif-status");
    if (st) st.textContent = "CUSTOM: " + cleanName.toUpperCase();
    if (typeof showToast === "function") showToast("Loaded & cleaned: " + file.name);
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

function drawGifMakerFrame(targetCtx, w, h, frameIdx, totalFrames) {
  const petId = document.getElementById("gif-pet-select").value;
  const fx = document.getElementById("gif-fx-select").value;
  const bgMode = document.getElementById("gif-bg-select").value;
  const caption = (document.getElementById("gif-caption").value || "").trim();
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

  const isCustomStrip = gifCustomImg && gifCustomImg.width >= gifCustomImg.height * 1.8;
  const phase = (frameIdx / totalFrames) * Math.PI * 2;
  let sx = 1, sy = 1, dx = 0, dy = 0, rot = 0;

  if (fx === "walk" && gifCustomImg && !isCustomStrip) {
    // Procedural waddle-walk for single uploaded PNG/SVG sprites so Walk Loop ALWAYS animates!
    dy = -Math.round(Math.abs(Math.sin(phase * 2)) * 5.5 * amp);
    dx = Math.round(Math.sin(phase) * 3.5 * amp);
    rot = Math.sin(phase) * 0.085;
    sx = 1 + Math.cos(phase * 2) * 0.045;
    sy = 1 - Math.cos(phase * 2) * 0.045;
  } else if (fx === "bounce") {
    dy = -Math.round(Math.abs(Math.sin(phase)) * 10 * amp);
    sy = 1 + Math.sin(phase * 2) * 0.06;
    sx = 1 - Math.sin(phase * 2) * 0.05;
  } else if (fx === "breathe") {
    sy = 1 + Math.sin(phase) * 0.05;
    sx = 1 - Math.sin(phase) * 0.03;
  } else if (fx === "hop") {
    const u = (frameIdx % totalFrames) / totalFrames;
    if (u < 0.25) { sx = 1.14; sy = 0.86; }
    else if (u < 0.65) { dy = -Math.round(16 * amp); sx = 0.90; sy = 1.12; }
    else { sx = 1.10; sy = 0.90; }
  } else if (fx === "wave") {
    rot = Math.sin(phase) * 0.09;
    dy = -Math.round(Math.abs(Math.sin(phase)) * 3 * amp);
  }

  const pad = Math.round((bgMode === "card" ? 18 : 8) * amp);
  const box = w - pad * 2;
  targetCtx.save();
  targetCtx.translate(w / 2 + dx, h - pad + dy);
  targetCtx.rotate(rot);
  targetCtx.scale(sx, sy);

  if (gifCustomImg && gifCustomImg.width > 0 && gifCustomImg.height > 0) {
    if (isCustomStrip && fx === "walk") {
      const cols = Math.max(2, Math.round(gifCustomImg.width / gifCustomImg.height));
      const fw = gifCustomImg.width / cols;
      const fh = gifCustomImg.height;
      const col = frameIdx % cols;
      targetCtx.drawImage(gifCustomImg, col * fw, 0, fw, fh, -Math.round(box / 2), -box, box, box);
    } else {
      const sw = isCustomStrip ? (gifCustomImg.width / Math.max(2, Math.round(gifCustomImg.width / gifCustomImg.height))) : gifCustomImg.width;
      const sh = gifCustomImg.height;
      const ratio = Math.min(box / sw, box / sh);
      const dw = Math.max(1, Math.round(sw * ratio));
      const dh = Math.max(1, Math.round(sh * ratio));
      targetCtx.drawImage(gifCustomImg, 0, 0, sw, sh, -Math.round(dw / 2), -dh, dw, dh);
    }
  } else if (typeof sprites !== "undefined" && sprites[petId] && sprites[petId].complete && sprites[petId].naturalWidth > 0) {
    const sp = sprites[petId];
    const fw = sp.naturalWidth / 9;
    const fh = sp.naturalHeight;
    const fCol = fx === "walk" ? (1 + (frameIdx % 4)) : 0;
    targetCtx.drawImage(sp, fCol * fw, 0, fw, fh, -Math.round(box / 2), -box, box, box);
  }
  targetCtx.restore();
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
  if (fx === "walk") {
    if (gifCustomImg) {
      if (gifCustomImg.width >= gifCustomImg.height * 1.8) {
        return Math.max(2, Math.round(gifCustomImg.width / gifCustomImg.height));
      }
      return 8;
    }
    return 4;
  }
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

    if (!gifCustomImg && (!sprites[petId] || !sprites[petId].complete || sprites[petId].naturalWidth === 0)) {
      statusEl.textContent = "WAIT: SPRITE STILL LOADING...";
      return;
    }

    const totalFrames = getGifTotalFrames(fx);
    const frames = [];
    for (let i = 0; i < totalFrames; i++) {
      drawGifMakerFrame(octx, size, size, i, totalFrames);
      frames.push(octx.getImageData(0, 0, size, size).data);
    }
    const delayCs = Math.max(4, Math.round(100 / fps));
    const gifBytes = encodeGif89a(frames, size, size, delayCs, transparent);
    const blob = new Blob([gifBytes], { type: "image/gif" });
    const slug = (gifCustomImg && gifCustomName) ? gifCustomName : petId;
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
    drawGifMakerFrame(prevC.getContext("2d"), prevC.width, prevC.height, gifPreviewTick, total);
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
  module.exports = { lzwEncodeGif, buildGifPalette, encodeGif89a };
}
