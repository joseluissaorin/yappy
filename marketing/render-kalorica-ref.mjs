#!/usr/bin/env node
/**
 * Marketing screenshots pipeline v2 — matches the refined cream-peach
 * aesthetic of the user's reference set, uses Apple's official
 * iPhone 17 Pro Max bezel PNG, and supports panorama bleed across
 * adjacent slots.
 *
 * Key differences from v1:
 *   - No hand-coded device frame (uses Apple's distributed PNG mockup)
 *   - Solid cream-peach background (NOT a gradient)
 *   - Big orange title with period (matching reference style)
 *   - Device bleeds off bottom of canvas for full-bleed look
 *   - Panorama support: 3-slot canvas with devices straddling boundaries
 *
 * Usage:
 *   node scripts/marketing-screenshots.mjs en
 *   node scripts/marketing-screenshots.mjs en --layout v2
 *
 * Inputs:
 *   marketing/layouts/<name>.json
 *   marketing/copy/<locale>.json
 *   marketing/screenshots/v1.5.0/<locale>/<screen_file>.png
 *   marketing/assets/apple-bezels/ProMax-Silver.png  (Apple official)
 *
 * Output:
 *   marketing/output/<locale>/01.png … 0N.png  (one per slot)
 */

import { readFileSync, mkdirSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import sharp from 'sharp';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO = join(__dirname, '..');

// CLI
const args = process.argv.slice(2);
const LOCALE = args[0] || 'en';
const LAYOUT = args.includes('--layout') ? args[args.indexOf('--layout') + 1] : 'v3';

const layoutPath = join(REPO, `marketing/layouts/${LAYOUT}.json`);
const copyPath = join(REPO, `marketing/copy/${LOCALE}.json`);
const rawDir = join(REPO, `marketing/screenshots/v1.5.0/${LOCALE}`);
const rawDirFallback = join(REPO, `marketing/screenshots/v1.5.0/en`);
const outDir = join(REPO, `marketing/output/${LOCALE}`);
const chipsPath = join(REPO, `marketing/chips/${LOCALE}.json`);

if (!existsSync(layoutPath)) throw new Error(`Layout not found: ${layoutPath}`);
if (!existsSync(copyPath)) throw new Error(`Copy not found: ${copyPath}`);
if (!existsSync(rawDir)) throw new Error(`Raw screenshots not found: ${rawDir}`);
mkdirSync(outDir, { recursive: true });

const layout = JSON.parse(readFileSync(layoutPath, 'utf8'));
const copy = JSON.parse(readFileSync(copyPath, 'utf8'));
const chips = existsSync(chipsPath)
  ? JSON.parse(readFileSync(chipsPath, 'utf8'))
  : {};

/**
 * Returns localized text for a card. If the card has a `card_id` and
 * the locale's chips file has an override for that id, use it. Otherwise
 * fall back to the layout's English text/subtext.
 */
function localizeCard(card) {
  if (!card.card_id) return card;
  const override = chips[card.card_id];
  if (!override) return card;
  return { ...card, text: override.text ?? card.text, subtext: override.subtext ?? card.subtext };
}

const G = layout.global;
const [SLOT_W, SLOT_H] = layout._meta.slot_size;
const BEZEL_PATH = join(REPO, G.device_bezel_png);
const [BEZEL_W, BEZEL_H] = G.device_bezel_dims;
const SCREEN_OFFSET = G.device_screen_offset; // {x, y, w, h} inside bezel PNG
const SCREEN_RADIUS = G.device_screen_corner_radius_px;

if (!existsSync(BEZEL_PATH)) throw new Error(`Bezel PNG not found: ${BEZEL_PATH}`);

const captionFor = (slot) =>
  copy.slots.find((s) => s.slot === slot) || { title: '', subtitle: '' };

// ── Helpers ──────────────────────────────────────────────────────────
const escapeXml = (s) =>
  String(s).replace(/[<>&"']/g, (c) => ({
    '<': '&lt;',
    '>': '&gt;',
    '&': '&amp;',
    '"': '&quot;',
    "'": '&apos;',
  }[c]));

const round = (n) => Math.round(n);

/**
 * Resize the raw screen to fit Apple's screen cutout dimensions, then
 * apply a rounded-rect mask so it sits cleanly behind the bezel.
 */
async function screenInsideBezel(screenPath) {
  const mask = Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${SCREEN_OFFSET.w}" height="${SCREEN_OFFSET.h}">
      <rect width="${SCREEN_OFFSET.w}" height="${SCREEN_OFFSET.h}" rx="${SCREEN_RADIUS}" fill="#fff"/>
    </svg>`,
  );
  return sharp(screenPath)
    .resize(SCREEN_OFFSET.w, SCREEN_OFFSET.h, { fit: 'cover' })
    .composite([{ input: mask, blend: 'dest-in' }])
    .png()
    .toBuffer();
}

/**
 * Compose a device (Apple bezel + raw screen content) at the requested
 * scale + rotation. Returns the rendered PNG buffer + final dims.
 */
async function renderDevice(spec) {
  let screenPath = join(rawDir, spec.screen_file);
  if (!existsSync(screenPath)) {
    // Fallback to en/ for files not yet captured per-locale
    // (e.g., gentle-home.png — same UI for all locales since gentle
    // mode hides numbers, leaving mostly graphical content).
    const fallback = join(rawDirFallback, spec.screen_file);
    if (existsSync(fallback)) {
      screenPath = fallback;
    } else {
      throw new Error(`Missing raw screenshot: ${screenPath}`);
    }
  }
  const screenBuf = await screenInsideBezel(screenPath);

  // Composite: blank transparent canvas → screen at offset → bezel on top
  const composed = await sharp({
    create: {
      width: BEZEL_W,
      height: BEZEL_H,
      channels: 4,
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    },
  })
    .composite([
      { input: screenBuf, top: SCREEN_OFFSET.y, left: SCREEN_OFFSET.x },
      { input: BEZEL_PATH },
    ])
    .png()
    .toBuffer();

  // Scale to requested size
  const targetW = round(BEZEL_W * spec.scale);
  const targetH = round(BEZEL_H * spec.scale);
  let buf = await sharp(composed).resize(targetW, targetH).png().toBuffer();

  // Rotate if needed
  if (spec.rotation_deg) {
    buf = await sharp(buf)
      .rotate(spec.rotation_deg, {
        background: { r: 0, g: 0, b: 0, alpha: 0 },
      })
      .png()
      .toBuffer();
  }

  const meta = await sharp(buf).metadata();
  return { buffer: buf, w: meta.width, h: meta.height };
}

/**
 * Caption SVG — big orange title with period + muted gray subtitle
 * underneath. Title can wrap on long words via tspan splitting.
 */
function captionSVG(title, subtitle, slotW, slotH, yStartPct) {
  const cx = slotW / 2;
  const titleY = slotH * yStartPct + G.title_size_px * 0.85;
  const subtitleY = titleY + G.title_size_px * 0.5 + G.subtitle_size_px * 0.8;

  // Naïve wrap: split subtitle by spaces into max ~30-char lines.
  const subLines = (() => {
    const words = subtitle.split(/\s+/);
    const lines = [];
    let cur = '';
    for (const w of words) {
      if ((cur + ' ' + w).trim().length > 32) {
        lines.push(cur.trim());
        cur = w;
      } else {
        cur = (cur + ' ' + w).trim();
      }
    }
    if (cur) lines.push(cur);
    return lines.slice(0, 3);
  })();

  const subTspans = subLines
    .map(
      (line, i) =>
        `<tspan x="${cx}" dy="${i === 0 ? 0 : G.subtitle_size_px * G.subtitle_line_height}">${escapeXml(line)}</tspan>`,
    )
    .join('');

  return Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${slotW}" height="${slotH}">
      <text x="${cx}" y="${titleY}" text-anchor="middle"
            font-family="${G.title_font_family}, -apple-system, Helvetica, sans-serif"
            font-size="${G.title_size_px}" font-weight="${G.title_font_weight}"
            fill="${G.title_color}" letter-spacing="${G.title_letter_spacing}">${escapeXml(title)}</text>
      <text x="${cx}" y="${subtitleY}" text-anchor="middle"
            font-family="${G.subtitle_font_family}, -apple-system, Helvetica, sans-serif"
            font-size="${G.subtitle_size_px}" font-weight="${G.subtitle_font_weight}"
            fill="${G.subtitle_color}">${subTspans}</text>
    </svg>`,
  );
}

/**
 * Solid-color background canvas of arbitrary width × SLOT_H.
 */
async function backgroundFor(canvasW, bg) {
  if (bg.type !== 'solid') throw new Error(`Unsupported bg type: ${bg.type}`);
  return sharp({
    create: { width: canvasW, height: SLOT_H, channels: 4, background: bg.color },
  })
    .png()
    .toBuffer();
}

/**
 * Soft pastel "blob" — a filled ellipse with low opacity, used as a
 * background accent behind devices. Color comes from Kalorica palette;
 * opacity tuned per-slot to avoid fighting the device for attention.
 */
function blobSVG(shape, canvasW) {
  const cx = round(canvasW * shape.x_pct);
  const cy = round(SLOT_H * shape.y_pct);
  return Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${canvasW}" height="${SLOT_H}">
      <defs>
        <filter id="soft" x="-20%" y="-20%" width="140%" height="140%">
          <feGaussianBlur stdDeviation="40"/>
        </filter>
      </defs>
      <ellipse cx="${cx}" cy="${cy}" rx="${shape.rx}" ry="${shape.ry}"
        fill="${shape.color}" opacity="${shape.opacity}" filter="url(#soft)"/>
    </svg>`,
  );
}

/**
 * Small accent dot — used for visual rhythm in the negative space.
 */
function dotSVG(shape, canvasW) {
  const cx = round(canvasW * shape.x_pct);
  const cy = round(SLOT_H * shape.y_pct);
  return Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${canvasW}" height="${SLOT_H}">
      <circle cx="${cx}" cy="${cy}" r="${shape.radius}" fill="${shape.color}"/>
    </svg>`,
  );
}

/**
 * Floating Kalorica-style mini-card chip. Solid pastel rounded rect with
 * dark text inside. Supports a primary `text` and optional `subtext`
 * (rendered below in slightly smaller weight). Drop shadow + optional
 * rotation give the "stuck-on-a-fridge" feel.
 *
 * Rendered to a tight-fit SVG canvas, then rotated via sharp so the
 * shadow/edges anti-alias cleanly.
 */
async function chipBuffer(chip) {
  const fontSize = chip.font_size_px || G.chip_default_font_size_px;
  const subSize = Math.round(fontSize * 0.55);
  const padX = G.chip_padding_x_px;
  const padY = G.chip_padding_y_px;
  const radius = G.chip_corner_radius_px;

  // Estimate chip dims from text. SpaceGrotesk-Bold ~ 0.55em char advance.
  const longest = Math.max(chip.text.length, (chip.subtext || '').length);
  const textW = Math.round(longest * fontSize * 0.58);
  const hasSub = !!chip.subtext;
  const innerH = hasSub ? fontSize * 1.05 + subSize * 1.1 : fontSize * 1.1;
  const chipW = textW + padX * 2;
  const chipH = Math.round(innerH + padY * 2);

  // Layout: title baseline at padY + fontSize, subtext below.
  const titleY = padY + fontSize * 0.85;
  const subY = titleY + fontSize * 0.5 + subSize * 0.6;

  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${chipW + 40}" height="${chipH + 40}" viewBox="0 0 ${chipW + 40} ${chipH + 40}">
    <defs>
      <filter id="chipShadow" x="-20%" y="-20%" width="140%" height="160%">
        <feGaussianBlur in="SourceAlpha" stdDeviation="6"/>
        <feOffset dx="0" dy="6"/>
        <feComponentTransfer><feFuncA type="linear" slope="${G.chip_shadow_alpha}"/></feComponentTransfer>
        <feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge>
      </filter>
    </defs>
    <g transform="translate(20 20)" filter="url(#chipShadow)">
      <rect x="0" y="0" width="${chipW}" height="${chipH}" rx="${radius}" fill="${chip.bg}"/>
      <text x="${chipW / 2}" y="${titleY}" text-anchor="middle"
            font-family="${G.chip_font_family}, -apple-system, Helvetica, sans-serif"
            font-size="${fontSize}" font-weight="${G.chip_font_weight}"
            fill="${G.chip_text_color}">${escapeXml(chip.text)}</text>
      ${
        hasSub
          ? `<text x="${chipW / 2}" y="${subY}" text-anchor="middle"
                font-family="${G.chip_font_family}, -apple-system, Helvetica, sans-serif"
                font-size="${subSize}" font-weight="500"
                fill="${G.chip_text_color}" opacity="0.7">${escapeXml(chip.subtext)}</text>`
          : ''
      }
    </g>
  </svg>`;

  let buf = await sharp(Buffer.from(svg)).png().toBuffer();
  if (chip.rotate_deg) {
    buf = await sharp(buf)
      .rotate(chip.rotate_deg, { background: { r: 0, g: 0, b: 0, alpha: 0 } })
      .png()
      .toBuffer();
  }
  const meta = await sharp(buf).metadata();
  return { buffer: buf, w: meta.width, h: meta.height };
}

/**
 * Build the composite list for background shapes + decorations of a
 * group (panorama or standalone). Background shapes go FIRST (behind
 * devices); decorations go LAST (in front).
 */
async function backgroundCompositesFor(group, canvasW) {
  const list = [];
  for (const shape of group.background_shapes || []) {
    if (shape.type === 'blob') list.push({ input: blobSVG(shape, canvasW), top: 0, left: 0 });
    else if (shape.type === 'dot') list.push({ input: dotSVG(shape, canvasW), top: 0, left: 0 });
    else if (shape.type === 'glyph') list.push(await glyphComposite(shape, canvasW));
    else if (shape.type === 'card') list.push(await cardComposite(localizeCard(shape), canvasW));
  }
  return list;
}

// cardComposite — Kalorica-style rounded card with optional text/subtext.
// Bigger and more structural than chip: explicit width/height in px,
// soft drop shadow, body text rendered with the title font. Used both
// as a background platform (behind device) and as a decoration (around
// device in a bento grid).
async function cardComposite(shape, canvasW) {
  const w = shape.width_px || 400;
  const h = shape.height_px || 300;
  const radius = shape.corner_radius_px || 40;
  const color = shape.color || '#FFD4A3';
  const opacity = shape.opacity ?? 1.0;
  const shadow = shape.shadow !== false;
  const text = shape.text || '';
  const subtext = shape.subtext || '';
  const fontSize = shape.font_size_px || 96;
  const subFontSize = shape.sub_font_size_px || Math.round(fontSize * 0.42);
  const textColor = shape.text_color || '#121212';
  const hasSub = !!subtext;
  const titleY = hasSub ? h / 2 - 8 : h / 2 + fontSize * 0.32;
  const subY = hasSub ? titleY + fontSize * 0.55 + subFontSize * 0.7 : 0;
  const pad = 60;
  const boxW = w + pad * 2;
  const boxH = h + pad * 2;
  const filterDef = shadow
    ? `<filter id="cardShadow" x="-20%" y="-20%" width="140%" height="160%">
        <feGaussianBlur in="SourceAlpha" stdDeviation="16"/>
        <feOffset dx="0" dy="12"/>
        <feComponentTransfer><feFuncA type="linear" slope="0.18"/></feComponentTransfer>
        <feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge>
      </filter>`
    : '';
  const filterAttr = shadow ? 'filter="url(#cardShadow)"' : '';
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${boxW}" height="${boxH}" viewBox="0 0 ${boxW} ${boxH}">
    <defs>${filterDef}</defs>
    <g transform="translate(${pad} ${pad})" ${filterAttr}>
      <rect x="0" y="0" width="${w}" height="${h}" rx="${radius}" fill="${color}" opacity="${opacity}"/>
      ${text ? `<text x="${w / 2}" y="${titleY}" text-anchor="middle"
            font-family="${G.title_font_family}, -apple-system, sans-serif"
            font-size="${fontSize}" font-weight="700" fill="${textColor}">${escapeXml(text)}</text>` : ''}
      ${hasSub ? `<text x="${w / 2}" y="${subY}" text-anchor="middle"
            font-family="${G.title_font_family}, -apple-system, sans-serif"
            font-size="${subFontSize}" font-weight="500" fill="${textColor}" opacity="0.7">${escapeXml(subtext)}</text>` : ''}
    </g>
  </svg>`;
  let buf = await sharp(Buffer.from(svg)).png().toBuffer();
  if (shape.rotate_deg) {
    buf = await sharp(buf)
      .rotate(shape.rotate_deg, { background: { r: 0, g: 0, b: 0, alpha: 0 } })
      .png()
      .toBuffer();
  }
  let meta = await sharp(buf).metadata();
  if (meta.width > canvasW || meta.height > SLOT_H) {
    buf = await sharp(buf)
      .resize({ width: canvasW - 2, height: SLOT_H - 2, fit: 'inside' })
      .png()
      .toBuffer();
    meta = await sharp(buf).metadata();
  }
  const cx = round(canvasW * shape.x_pct);
  const cy = round(SLOT_H * shape.y_pct);
  const ax = shape.anchor || 'center';
  const ay = shape.anchor_y || 'center';
  let left;
  if (ax === 'left') left = cx;
  else if (ax === 'right') left = cx - meta.width;
  else left = cx - meta.width / 2;
  let top;
  if (ay === 'top') top = cy;
  else if (ay === 'bottom') top = cy - meta.height;
  else top = cy - meta.height / 2;
  left = Math.max(0, Math.min(canvasW - meta.width, round(left)));
  top = Math.max(0, Math.min(SLOT_H - meta.height, round(top)));
  return { input: buf, top, left };
}

// glyphComposite — renders ONE oversized typographic mark in a pastel
// at low opacity as a background decoration. Returns a sharp composite
// entry { input, top, left }. The glyph is rotated then center-anchored
// at (x_pct, y_pct) of the canvas.
async function glyphComposite(shape, canvasW) {
  const fontSize = shape.font_size_px || 800;
  // Generous box so descenders/diacritics aren't clipped when rotated.
  // Clamp to canvas dimensions — sharp's composite refuses buffers
  // bigger than the destination. Glyphs that exceed canvas just get
  // cropped at the box, which mimics the bleed-off-edge intent anyway.
  const box = Math.min(
    Math.round(fontSize * 1.6),
    canvasW - 1,
    SLOT_H - 1,
  );
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${box}" height="${box}" viewBox="0 0 ${box} ${box}">
    <text x="50%" y="50%" text-anchor="middle" dominant-baseline="central"
          font-family="${G.title_font_family}, -apple-system, Helvetica, sans-serif"
          font-size="${fontSize}" font-weight="${G.title_font_weight}"
          fill="${shape.color}" opacity="${shape.opacity ?? 0.25}">${escapeXml(shape.text)}</text>
  </svg>`;
  let buf = await sharp(Buffer.from(svg)).png().toBuffer();
  if (shape.rotate_deg) {
    buf = await sharp(buf)
      .rotate(shape.rotate_deg, { background: { r: 0, g: 0, b: 0, alpha: 0 } })
      .png()
      .toBuffer();
  }
  // Rotation can expand the buffer past the canvas — sharp refuses to
  // composite anything bigger than the destination. Resize down if
  // needed, preserving aspect.
  let meta = await sharp(buf).metadata();
  if (meta.width > canvasW || meta.height > SLOT_H) {
    buf = await sharp(buf)
      .resize({ width: canvasW - 2, height: SLOT_H - 2, fit: 'inside' })
      .png()
      .toBuffer();
    meta = await sharp(buf).metadata();
  }
  const cx = round(canvasW * shape.x_pct);
  const cy = round(SLOT_H * shape.y_pct);
  return {
    input: buf,
    top: round(cy - meta.height / 2),
    left: round(cx - meta.width / 2),
  };
}

async function decorationCompositesFor(group, canvasW) {
  const list = [];
  for (const d of group.decorations || []) {
    if (d.type === 'dot') {
      list.push({ input: dotSVG(d, canvasW), top: 0, left: 0 });
    } else if (d.type === 'card') {
      list.push(await cardComposite(localizeCard(d), canvasW));
    } else if (d.type === 'glyph') {
      list.push(await glyphComposite(d, canvasW));
    } else if (d.type === 'chip') {
      const { buffer, w, h } = await chipBuffer(d);
      const cx = round(canvasW * d.x_pct);
      const cy = round(SLOT_H * d.y_pct);
      // anchor: where on the chip the (x_pct, y_pct) point lands.
      // Default 'center'. 'left' = chip's left edge is at cx. 'right' =
      // chip's right edge is at cx. Same axis for y via anchorY.
      const ax = d.anchor || 'center';
      const ay = d.anchor_y || 'center';
      let left;
      if (ax === 'left') left = cx;
      else if (ax === 'right') left = cx - w;
      else left = cx - w / 2;
      let top;
      if (ay === 'top') top = cy;
      else if (ay === 'bottom') top = cy - h;
      else top = cy - h / 2;
      // Clamp into canvas so chips never bleed beyond the image bounds.
      left = Math.max(0, Math.min(canvasW - w, round(left)));
      top = Math.max(0, Math.min(SLOT_H - h, round(top)));
      list.push({ input: buffer, top, left });
    }
  }
  return list;
}

// ── Panorama group ───────────────────────────────────────────────────
async function renderPanorama(group) {
  const slots = group.slots;
  const canvasW = group.canvas[0];
  console.log(`  → panorama ${slots.join(',')} (${canvasW}×${SLOT_H})`);

  const bg = await backgroundFor(canvasW, group.background);
  const composites = [];

  // Z-order: background shapes first (behind devices), then devices,
  // then chip/dot decorations on top, then captions over everything.
  composites.push(...(await backgroundCompositesFor(group, canvasW)));

  // Devices
  for (const dev of group.devices) {
    const { buffer, w, h } = await renderDevice(dev);
    const cx = round(canvasW * dev.x_pct);
    const top = round(SLOT_H * dev.top_y_pct);
    composites.push({
      input: buffer,
      top,
      left: round(cx - w / 2),
    });
  }

  // Decorations (chips, dots) on top of devices for that scrapbook feel
  composites.push(...(await decorationCompositesFor(group, canvasW)));

  // Captions on top of everything
  for (const slot of slots) {
    const cap = captionFor(slot);
    if (!cap.title) continue;
    const slotX = (slot - slots[0]) * SLOT_W;
    composites.push({
      input: captionSVG(cap.title, cap.subtitle, SLOT_W, SLOT_H, G.caption_y_start_pct),
      top: 0,
      left: slotX,
    });
  }

  const panorama = await sharp(bg).composite(composites).png().toBuffer();

  // Slice into per-slot PNGs
  for (let i = 0; i < slots.length; i++) {
    const slot = slots[i];
    const outPath = join(outDir, `${String(slot).padStart(2, '0')}.png`);
    await sharp(panorama)
      .extract({ left: i * SLOT_W, top: 0, width: SLOT_W, height: SLOT_H })
      .png()
      .toFile(outPath);
    console.log(`    ✓ slot ${slot} → ${outPath}`);
  }
}

// ── Standalone group ─────────────────────────────────────────────────
async function renderStandalone(group) {
  const slot = group.slot;
  console.log(`  → standalone ${slot}`);

  const bg = await backgroundFor(SLOT_W, group.background);
  const composites = [];

  // 1. Background shapes (blobs) behind device
  composites.push(...(await backgroundCompositesFor(group, SLOT_W)));

  // 2. Device
  const { buffer, w, h } = await renderDevice(group.device);
  const cx = round(SLOT_W * group.device.x_pct);
  const top = round(SLOT_H * group.device.top_y_pct);
  composites.push({
    input: buffer,
    top,
    left: round(cx - w / 2),
  });

  // 3. Decorations (chips, dots) in front of device
  composites.push(...(await decorationCompositesFor(group, SLOT_W)));

  // 4. Caption on top
  const cap = captionFor(slot);
  if (cap.title) {
    composites.push({
      input: captionSVG(cap.title, cap.subtitle, SLOT_W, SLOT_H, G.caption_y_start_pct),
      top: 0,
      left: 0,
    });
  }

  const outPath = join(outDir, `${String(slot).padStart(2, '0')}.png`);
  await sharp(bg).composite(composites).png().toFile(outPath);
  console.log(`    ✓ slot ${slot} → ${outPath}`);
}

// ── Main ─────────────────────────────────────────────────────────────
console.log(`Marketing screenshots v2 · layout=${LAYOUT} · locale=${LOCALE}`);
console.log(`  bezel:  ${BEZEL_PATH}`);
console.log(`  raw:    ${rawDir}`);
console.log(`  output: ${outDir}\n`);

for (const group of layout.groups) {
  if (group.type === 'panorama') await renderPanorama(group);
  else if (group.type === 'standalone') await renderStandalone(group);
}
console.log('\nDone.');
