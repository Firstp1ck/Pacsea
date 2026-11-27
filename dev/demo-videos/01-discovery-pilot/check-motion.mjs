// Run with Node 22+, the explainer-video skill directory and optionally a muxed MP4 path.
import { once } from 'node:events';
import { execFileSync, spawn } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

if (!process.argv[2]) throw new Error('Pass the explainer-video skill directory.');
const library = resolve(process.argv[2], 'scripts/lib/storyboard.mjs');
const { loadStoryboard, buildPage, validateStoryboard } = await import(pathToFileURL(library));
const loaded = loadStoryboard(join(import.meta.dirname, 'storyboard.json'));
const { sb } = loaded;
const starts = [];
let duration = 0;
for (const step of sb.steps) {
  starts.push(duration);
  duration += step.duration;
}
const cards = {
  'discovery-intro': { start: 0, duration: 8, capture: 'browse' },
  'build-install-list': { start: 122, duration: 8.25, capture: 'namcap' },
  'end-credits': { start: 202, duration: 9, capture: 'list-final' }
};
for (const [scene, expected] of Object.entries(cards)) {
  const i = sb.steps.findIndex(step => step.scene === scene);
  if (starts[i] !== expected.start || sb.steps[i].duration !== expected.duration) {
    throw new Error('Incorrect title timing for ' + scene);
  }
}
if (duration !== 211 || sb.fps !== 24 || sb.chapters.length !== 5 ||
    sb.chapters.some(c => !starts.includes(c.start)) || sb.steps.at(-1).scene !== 'end-credits') {
  throw new Error('Expected five chapters and a closing title in the 211-second pilot.');
}
execFileSync('python', [join(import.meta.dirname, 'check-targets.py')], { stdio: 'inherit' });
const validation = validateStoryboard(sb, { scenes: loaded.scenes, dir: loaded.dir });
if (validation.errors.length) throw new Error(validation.errors.join(', '));

// This function runs inside the renderer page so font metrics, actions and cut
// ownership come from Chromium. Coordinates are output pixels at native size.
async function measure(storyboard, cards) {
  const { total, fps } = await window.__EV.ready();
  const modalBounds = {
    vote: [240, 267, 1440, 580], import: [272, 267, 1360, 551],
    help: [192, 151, 1536, 812], preflight: [192, 180, 1536, 754],
    'preflight-wait': [192, 180, 1536, 754], 'sort-menu': [256, 122, 288, 145]
  };
  const patches = [], patchIds = new Map();
  const canvas = document.createElement('canvas');
  const context = canvas.getContext('2d', { willReadFrequently: true });
  function glyphPatch(img, native, spot) {
    const [x, y, w, h] = spot.rect;
    const key = img.src + '/' + spot.rect;
    if (patchIds.has(key)) return patchIds.get(key);
    canvas.width = w; canvas.height = h;
    context.drawImage(img, x - native.x, y - native.y, w, h, 0, 0, w, h);
    const pixels = context.getImageData(0, 0, w, h).data;
    const samples = [];
    // Sample both ink and background across the string. Comparing these to native
    // luma catches a stale capture even if the cyan outline itself is in place.
    // The 14-pixel outline glow reaches into the rounded corners.
    for (let col = 7; col < w - 7; col += Math.max(16, Math.floor(w / 24))) {
      let light, dark;
      for (let px = col; px < Math.min(col + 16, w - 7); px++) {
        for (let py = 7; py < h - 7; py++) {
          const i = (py * w + px) * 4;
          const luma = Math.round(16 + (65.481 * pixels[i] + 128.553 * pixels[i + 1] + 24.966 * pixels[i + 2]) / 255);
          const sample = [x + px, y + py, luma];
          if (!light || luma > light[2]) light = sample;
          if (!dark || luma < dark[2]) dark = sample;
        }
      }
      if (light[2] - dark[2] > 28) samples.push(light, dark);
    }
    if (!samples.length) throw new Error('No rendered glyph contrast in ' + key);
    const index = patches.length;
    patches.push({ text: spot.anchor[1], samples, rect: spot.rect });
    patchIds.set(key, index);
    return index;
  }
  const issues = new Map();
  const frames = [];
  const hosts = [...document.querySelectorAll('.ev-step')];
  const captures = new Set();
  let worst = 0, worstFrame = 0, worstCut = 0, cuts = 0, movingFrames = 0;
  let previous, panelSize, start = 0;
  for (const [index, step] of storyboard.steps.entries()) {
    const host = hosts[index];
    const scene = host.querySelector('.ev-scene');
    const images = [...scene.querySelectorAll('.terminal')];
    const boundary = Math.round(start * fps);
    if (step.transition !== 'cut' || step.caption.track?.some(k => k.move <= 0 || k.transition === 'fade')) {
      throw new Error('Expected a cut and visible caption glides in ' + step.scene);
    }
    for (let frame = boundary; frame < Math.round((start + step.duration) * fps); frame++) {
      const t = frame / fps;
      const local = t - start;
      window.__EV.seek(t);
      const report = message => {
        const key = step.scene + ' / ' + message;
        if (!issues.has(key)) issues.set(key, { first: t, count: 0 });
        issues.get(key).count++;
      };
      const panels = document.querySelectorAll('.ev-cap-placed');
      if (panels.length !== 1) throw new Error('Wrong caption count at ' + t);
      const panel = panels[0], box = panel.getBoundingClientRect();
      const img = images.findLast(el => getComputedStyle(el).visibility === 'visible');
      if (!img || getComputedStyle(host).opacity !== '1') throw new Error('Missing scene at ' + t);
      const native = img.getBoundingClientRect(), stage = scene.getBoundingClientRect();
      if (!img.complete || img.naturalWidth !== 1856 ||
          [native.x, native.y, native.width, native.height].join() !== '32,64,1856,986' ||
          [stage.x, stage.y, stage.width, stage.height].join() !== '0,0,1920,1080') report('window geometry');
      const capture = img.src.split('/').at(-1).replace(/\.svg$/, '');
      captures.add(capture);
      const spots = step.spotlights.filter(s => local >= s.from && local < s.to);
      const released = step.scene === 'import-list' && local >= 7.25 && capture === 'export-base';
      if (spots.length !== (released ? 0 : 1)) throw new Error('Expected one spotlight at ' + t);
      if (cards[step.scene]) {
        const copy = scene.querySelector('.title-copy');
        const scrim = scene.querySelector('.title-scrim').getBoundingClientRect();
        if (box.width || box.height || capture !== cards[step.scene].capture ||
            [scrim.x, scrim.y, scrim.width, scrim.height].join() !== '0,0,1920,1080') {
          report('title coverage, base capture or competing caption');
        }
        if (local >= 1.25 && local <= step.duration - 1.25 &&
            (getComputedStyle(copy).opacity !== '1' || getComputedStyle(copy).transform !== 'none')) {
          report('moving or faded copy during reading hold');
        }
        if (local === 2) {
          const walker = document.createTreeWalker(copy, NodeFilter.SHOW_TEXT);
          const ink = [];
          while (walker.nextNode()) {
            const node = walker.currentNode;
            if (!node.textContent.trim()) continue;
            const range = document.createRange();
            range.selectNodeContents(node);
            for (const r of range.getClientRects()) {
              if (r.width < 1) continue;
              if (r.left < 72 || r.right > storyboard.width - 72 || r.top < 54 ||
                  r.bottom > storyboard.height - 54 || r.height < 30) report('title glyph size or clearance');
              ink.push([r.x, r.y, r.width, r.height].map(Math.round));
            }
          }
          cards[step.scene].ink = ink;
          const heading = copy.querySelector('h1').getBoundingClientRect();
          cards[step.scene].heading = [heading.x, heading.y, heading.width, heading.height].map(Math.round);
        }
        // A title interrupts the caption track. Every numbered shot still gets
        // its original per-frame outline, clearance, speed and cut checks.
        frames.push(null);
        previous = null;
        continue;
      }
      const size = [box.width, box.height].join(',');
      panelSize ??= size;
      if (size !== panelSize || getComputedStyle(panel).opacity !== '1' ||
          box.x < 0 || box.y < 0 || box.right > storyboard.width || box.bottom > storyboard.height) {
        report('panel size, opacity or bounds');
      }
      for (const span of panel.querySelectorAll('.ev-bold, .ev-rest')) {
        const range = document.createRange();
        range.selectNodeContents(span);
        if (range.getClientRects().length !== 1 || range.getBoundingClientRect().right > box.right - 8) {
          report('caption text overflow');
        }
      }
      const overlaps = ([x, y, width, height], pad = 0) =>
        box.x < x + width + pad && x - pad < box.right && box.y < y + height + pad && y - pad < box.bottom;
      if (modalBounds[capture] && overlaps(modalBounds[capture], 2)) report('dialog overlap');
      if (spots.length && overlaps(spots[0].rect, 9)) report('spotlight halo overlap');
      const delta = previous ? Math.hypot(box.x - previous.x, box.y - previous.y) : 0;
      if (delta > worst) { worst = delta; worstFrame = frame; }
      if (delta > 0) movingFrames++;
      if (previous && frame === boundary) {
        cuts++;
        worstCut = Math.max(worstCut, delta);
        if (delta !== 0) report('jump at cut');
      }
      if (delta > 34) report('speed over 34 px/frame');
      const spot = spots[0];
      const moving = step.caption.track?.some(k => k.at - k.move <= local + 1e-6 && local < k.at);
      if (spot && !moving) {
        const [sx, sy, sw, sh] = spot.rect;
        const gap = Math.hypot(Math.max(0, sx - box.right, box.left - sx - sw),
          Math.max(0, sy - box.bottom, box.top - sy - sh));
        if (gap > 215) report('caption parked away from ' + spot.anchor[1]);
      }
      frames.push([box.x, box.y, spot ? glyphPatch(img, native, spot) : -1]);
      previous = box;
    }
    start += step.duration;
  }
  for (const [id, card] of Object.entries(cards)) {
    const scene = hosts[storyboard.steps.findIndex(step => step.scene === id)];
    const sample = () => {
      window.__EV.seek(card.start + .5);
      return [...scene.querySelectorAll('.title-scrim,.title-copy,.sweep,.end-fade')]
        .map(el => el.style.opacity + el.style.transform).join('|');
    };
    window.__EV.seek(card.start);
    const forward = sample();
    window.__EV.seek(card.start + card.duration - .25);
    if (sample() !== forward) throw new Error(id + ' motion depends on seek order');
  }
  return { frameCount: frames.length, total, fps, cuts, captures: captures.size, panelSize,
    worst, worstFrame, worstCut, movingFrames, issues: Object.fromEntries(issues), frames, patches, cards };
}

// The straight edges identify the panel in decoded luma without relying on
// anti-aliased text. Probe several points to tolerate isolated codec noise.
function locateEdge(guess, sample) {
  let best = { position: -1, brightness: -1 };
  for (let pos = Math.round(guess) - 3; pos <= Math.round(guess) + 3; pos++) {
    const brightness = sample(pos).sort((a, b) => a - b)[2];
    if (brightness > best.brightness) best = { position: pos, brightness };
  }
  return best;
}

// Measure travel from decoded panel edges. Interpolate between adjacent frames
// so fractional key times are compared at the same point in each glide.
function checkTravel(positions) {
  let glides = 0, firstGlidePercent;
  for (const [i, step] of sb.steps.entries()) {
    if (cards[step.scene]) continue;
    let [x, y] = step.caption.box;
    let previousTime = 0;
    for (const key of step.caption.track ?? []) {
      const move = Math.min(key.move, key.at - previousTime);
      const start = starts[i] + key.at - move;
      const dx = key.x - x, dy = key.y - y;
      const distance = Math.hypot(dx, dy);
      if (distance > 0) {
        let last = 0;
        for (let frame = Math.ceil(start * sb.fps); frame <= Math.floor((start + move) * sb.fps); frame++) {
          const px = positions[frame].x - x, py = positions[frame].y - y;
          const forward = (px * dx + py * dy) / distance;
          const side = Math.abs(px * dy - py * dx) / distance;
          if (side > 1.6 || forward < last - 1.6 || forward > distance + 1.6) {
            throw new Error(step.scene + ' caption reversed or left its direct route at frame ' + frame);
          }
          last = Math.max(last, forward);
        }
        const progress = [0.25, 0.5, 0.75].map(fraction => {
          const frame = (start + move * fraction) * sb.fps;
          const lower = Math.floor(frame), mix = frame - lower;
          const a = positions[lower], b = positions[lower + 1];
          const px = a.x + mix * (b.x - a.x), py = a.y + mix * (b.y - a.y);
          return ((px - x) * dx + (py - y) * dy) / (distance * distance);
        });
        // The early velocity peak puts roughly 36%, 76% and 96% of the
        // distance behind the panel. Short trips also need pixel tolerance.
        const expected = [0.36, 0.76, 0.96], tolerance = 0.025 + 1.5 / distance;
        if (progress.some((p, j) => Math.abs(p - expected[j]) > tolerance)) {
          throw new Error(step.scene + ' glide ending at ' + key.at + 's: ' + progress.join(', '));
        }
        firstGlidePercent ??= progress.map(p => Number((p * 100).toFixed(1)));
        glides++;
      }
      x = key.x;
      y = key.y;
      previousTime = key.at;
    }
  }
  return { glides, firstGlidePercent };
}

function checkTitleSamples(titleSamples) {
  for (const [scene, samples] of Object.entries(titleSamples)) {
    const hold = samples.slice(Math.ceil(1.25 * sb.fps), -Math.ceil(1.25 * sb.fps));
    if (hold.length < 5 * sb.fps || !hold[0].ink.length ||
        hold.some(sample => sample.ink.some(fraction => fraction < .025))) {
      throw new Error(scene + ' is missing legible text during its reading hold');
    }
    for (const [region, first] of samples[0].backdrop.entries()) {
      const levels = samples.map(sample => sample.backdrop[region]);
      // Luma 16 represents black in limited-range H.264.
      if (hold.some(sample => sample.backdrop[region] - 16 > .60 * (first - 16) + .5) ||
          new Set(levels.slice(0, .75 * sb.fps).map(v => v.toFixed(1))).size < 5 ||
          levels.slice(1, .75 * sb.fps).some((v, i) => v > levels[i] + 1.2)) {
        throw new Error(scene + ' has an incomplete dimming ramp in region ' + region);
      }
      if (scene !== 'end-credits') {
        const release = levels.slice(-.75 * sb.fps);
        if (Math.abs(release.at(-1) - first) > 4 ||
            new Set(release.map(v => v.toFixed(1))).size < 5 ||
            release.some((v, i) => i && v < release[i - 1] - 1.2)) {
          throw new Error(scene + ' did not brighten back to the app in region ' + region);
        }
      }
    }
    // A binary ink threshold can switch off most letters in a single frame.
    // Integrate headline luminance to measure the actual fade between frames.
    const headline = samples.map(sample => sample.headline);
    const holdLevel = hold[0].headline;
    const entry = headline.slice(0, sb.fps), exit = headline.slice(-sb.fps * 1.25);
    if (headline[0] > .1 * holdLevel || headline.at(-1) > .1 * holdLevel ||
        new Set(entry.map(v => v.toFixed(1))).size < 7 ||
        new Set(exit.map(v => v.toFixed(1))).size < 7) {
      throw new Error(scene + ' has an incomplete headline ramp: ' +
        JSON.stringify({ entry, holdLevel, exit }));
    }
    if (scene === 'end-credits' && samples.at(-1).backdrop.some(v => v > 21)) {
      throw new Error('Closing frame did not resolve to black');
    }
  }
}

function checkScore(movie) {
  const score = join(import.meta.dirname, 'audio/quiet-index.m4a');
  const hashes = [movie, score].map(file => {
    const fields = 'stream=codec_name,sample_rate,channels,start_time,duration_ts,time_base';
    const probe = JSON.parse(execFileSync('ffprobe', ['-v', 'error', '-select_streams', 'a:0',
      '-show_entries', fields, '-of', 'json', file]));
    const stream = probe.streams[0];
    if (stream?.codec_name !== 'aac' || stream.sample_rate !== '48000' || stream.channels !== 2 ||
        Number(stream.start_time) !== 0 || stream.time_base !== '1/48000' || stream.duration_ts !== duration * 48000) {
      throw new Error('Score timing or format differs from the episode in ' + file);
    }
    return execFileSync('ffmpeg', ['-nostdin', '-v', 'error', '-xerror', '-i', file,
      '-map', '0:a:0', '-c:a', 'copy', '-f', 'streamhash', '-hash', 'sha256', '-'], { encoding: 'utf8' }).trim();
  });
  if (hashes[0] !== hashes[1]) throw new Error('Mux changed the native AAC score');
  console.log(hashes[0]);
}

async function checkVideo(frames, titles, patches) {
  const path = resolve(process.argv[3] ?? join(import.meta.dirname, 'video.mp4'));
  const fields = 'stream=codec_type,codec_name,width,height,pix_fmt,avg_frame_rate,time_base:frame=best_effort_timestamp:chapter=start_time,end_time:chapter_tags=title:format=duration';
  const probe = JSON.parse(execFileSync('ffprobe', ['-v', 'error', '-select_streams', 'v:0', '-show_entries', fields,
    '-show_frames', '-of', 'json', path], { maxBuffer: 8 << 20 }));
  const stream = probe.streams.find(s => s.codec_type === 'video');
  const [numerator, denominator] = stream.time_base.split('/').map(Number);
  if (stream.codec_name !== 'h264' || stream.width !== sb.width || stream.height !== sb.height ||
      stream.pix_fmt !== 'yuv420p' || stream.avg_frame_rate !== sb.fps + '/1' ||
      Number(probe.format.duration) !== duration ||
      probe.frames.length !== frames.length || probe.frames.some((f, i) =>
        Number(f.best_effort_timestamp) * numerator * sb.fps !== i * denominator)) {
    throw new Error('MP4 format, duration or presentation timestamps differ from the storyboard.');
  }
  if (probe.chapters.length !== sb.chapters.length || probe.chapters.some((c, i) =>
    Number(c.start_time) !== sb.chapters[i].start || c.tags.title !== sb.chapters[i].title ||
    Number(c.end_time) !== (sb.chapters[i + 1]?.start ?? duration))) throw new Error('Incorrect MP4 chapters.');

  checkScore(path);
  const ySize = stream.width * stream.height;
  const buffer = Buffer.allocUnsafe(ySize * 3 / 2);
  const decoder = spawn('ffmpeg', ['-nostdin', '-v', 'error', '-xerror', '-i', path,
    '-map', '0:v:0', '-fps_mode', 'passthrough', '-f', 'rawvideo', '-pix_fmt', 'yuv420p', '-'],
    { stdio: ['ignore', 'pipe', 'inherit'] });
  const finished = once(decoder, 'close');
  const cutFrames = new Set(starts.slice(1).map(t => t * sb.fps));
  const positions = [];
  let used = 0, frame = 0, previous, worstDelta = 0, worstCut = 0, worstError = 0;
  const failures = new Map();
  const issue = type => {
    if (!failures.has(type)) failures.set(type, { firstFrame: frame, count: 0 });
    failures.get(type).count++;
  };
  const yAt = (x, y) => buffer[y * stream.width + x];
  const cyanAt = (x, y, minLuma = 154) => {
    const chroma = (y >> 1) * (stream.width >> 1) + (x >> 1);
    return yAt(x, y) > minLuma && buffer[ySize + chroma] > 136 && buffer[ySize * 1.25 + chroma] < 109;
  };
  const titleSamples = {};
  const titleAtFrame = [];
  for (const [scene, card] of Object.entries(titles)) {
    titleSamples[scene] = [];
    for (let f = card.start * sb.fps; f < (card.start + card.duration) * sb.fps; f++) titleAtFrame[f] = scene;
  }
  function sampleRect([x, y, w, h], brightness) {
    let total = 0, samples = 0;
    for (let row = y; row < y + h; row += 3) {
      for (let col = x; col < x + w; col += 3) {
        total += brightness(yAt(col, row));
        samples++;
      }
    }
    return total / samples;
  }
  for await (const chunk of decoder.stdout) {
    let offset = 0;
    while (offset < chunk.length) {
      const size = Math.min(buffer.length - used, chunk.length - offset);
      chunk.copy(buffer, used, offset, offset + size);
      offset += size;
      used += size;
      if (used !== buffer.length) continue;
      used = 0;
      const title = titleAtFrame[frame];
      if (title) {
        const card = titles[title];
        const step = sb.steps.find(step => step.scene === title);
        const [sx, sy, sw, sh] = step.spotlights[0].rect;
        // Sample the four outer corners and the lit outline separately; a
        // lower outline stays clear of the entering headline throughout its ramp.
        const regions = [[0, 0, 180, 72], [1740, 0, 180, 72], [0, 1008, 180, 72],
          [1740, 1008, 180, 72], [sx + 10, sy + sh + 1, sw - 20, 1]];
        titleSamples[title].push({
          backdrop: regions.map(r => sampleRect(r, y => y)),
          ink: card.ink.map(r => sampleRect(r, y => Number(y > 135))),
          headline: sampleRect(card.heading, y => Math.max(0, y - 65))
        });
        positions.push(null);
        previous = null;
        frame++;
        continue;
      }
      const [x, y, patch] = frames[frame];
      const left = locateEdge(x, col => [16, 27, 43, 60, 73].map(d => yAt(col, Math.round(y) + d)));
      const top = locateEdge(y, row => [30, 102, 233, 378, 501].map(d => yAt(Math.round(x) + d, row)));
      if (Math.min(left.brightness, top.brightness) < 77 ||
          Math.abs(yAt(left.position + 8, top.position + 45) - 40) > 12) issue('missing caption panel');
      const error = Math.max(Math.abs(left.position - x), Math.abs(top.position - y));
      worstError = Math.max(worstError, error);
      if (error > 1.1) issue('caption position');
      const delta = previous ? Math.hypot(left.position - previous.x, top.position - previous.y) : 0;
      worstDelta = Math.max(worstDelta, delta);
      if (cutFrames.has(frame)) worstCut = Math.max(worstCut, delta);
      previous = { x: left.position, y: top.position };
      positions.push(previous);
      if (patch < 0) {
        // The last half-second of the dismissed importer is deliberately unlit.
        let cyan = 0;
        for (let row = 2; row < stream.height; row += 3) {
          for (let col = 2; col < stream.width; col += 3) if (cyanAt(col, row)) cyan++;
        }
        if (cyan > 3) issue('outline remained after dialog close');
        frame++;
        continue;
      }
      const { rect: [sx, sy, sw], text, samples } = patches[patch];
      const wrong = samples.filter(([px, py, luma]) => Math.abs(yAt(px, py) - luma) > 22);
      if (wrong.length > samples.length * .22) issue('native glyph mismatch: ' + text);
      const bright = [0.18, 0.34, 0.5, 0.66, 0.82].filter(fraction => {
        const col = Math.round(sx + fraction * sw), row = sy - 2;
        return cyanAt(col, row);
      });
      if (bright.length < 3) issue('missing cyan outline');
      frame++;
    }
  }
  const [code] = await finished;
  if (code || used || frame !== frames.length) throw new Error('Incomplete video decode.');
  checkTitleSamples(titleSamples);
  const metrics = { decodedFrames: frame, worstDecodedDelta: worstDelta, worstDecodedCut: worstCut,
    maxEdgeError: worstError, issues: Object.fromEntries(failures) };
  if (!failures.size) Object.assign(metrics, checkTravel(positions));
  console.log(JSON.stringify(metrics, null, 2));
  if (failures.size || worstCut !== 0 || worstDelta > 34) process.exitCode = 1;
}

const directory = mkdtempSync(join(tmpdir(), 'pacsea-motion-'));
try {
  const audit = '(' + measure + ')(' + JSON.stringify(sb).replaceAll('<', '\\u003c') + ',' + JSON.stringify(cards) + ')';
  const output = 'value => document.body.replaceChildren(Object.assign(document.createElement("pre"), {textContent: JSON.stringify(value)}))';
  const script = '<script>' + audit + '.catch(e => ({error: String(e)})).then(' + output + ')</script>';
  const page = join(directory, 'check.html');
  writeFileSync(page, buildPage(loaded).replace('</body>', script + '</body>'));
  const dom = execFileSync(process.env.CHROME_PATH || 'chromium', ['--headless=new', '--disable-gpu',
    '--no-first-run', '--disable-background-networking', '--allow-file-access-from-files',
    '--font-render-hinting=none', '--force-device-scale-factor=1', '--window-size=1920,1080', '--user-data-dir=' + join(directory, 'profile'),
    '--virtual-time-budget=60000', '--dump-dom', pathToFileURL(page).href],
    { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], timeout: 150000, maxBuffer: 32 << 20 });
  const match = dom.match(/<pre>([^<]*)<\/pre>/);
  if (!match) throw new Error('Chromium did not return frame measurements.');
  const result = JSON.parse(match[1]);
  if (result.error) throw new Error(result.error);
  const { frames, patches, cards: titles, ...metrics } = result;
  console.log(JSON.stringify(metrics, null, 2));
  if (result.frameCount !== Math.round(duration * sb.fps) || Object.keys(result.issues).length) process.exitCode = 1;
  if (!process.exitCode && !process.argv.includes('--layout-only')) await checkVideo(frames, titles, patches);
} finally {
  rmSync(directory, { recursive: true, force: true, maxRetries: 3 });
}
