// Generates small synthetic test videos in samples/ (Hi10P H.264 MKV + ASS/SRT, HEVC 10-bit MKV, H.264 MP4) for the player spike and tests.
//
// Usage: node scripts/make-samples.mjs <path-to-ffmpeg-with-libx264-and-libx265>
// The bundled LGPL ffmpeg has no x264/x265 encoders, so pass a full (GPL) build; the samples are
// local test data only (samples/ is gitignored) and are never shipped.
import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const OUT = join(ROOT, 'samples');
const ffmpeg = process.argv[2];
if (!ffmpeg) {
  console.error('usage: node scripts/make-samples.mjs <ffmpeg.exe with libx264/libx265>');
  process.exit(2);
}
mkdirSync(OUT, { recursive: true });

/** Dialogue lines: [start s, end s, Japanese, English]. */
const LINES = [
  [1.0, 4.0, 'もう諦めないって決めたんだ。', "I decided I'm not giving up anymore."],
  [4.5, 8.0, '食べさせられなかったのは残念だね。', "It's a shame we couldn't make them eat it."],
  [9.0, 12.0, '明日、一緒に行こうよ。', "Let's go together tomorrow."],
  [12.5, 16.0, '夕焼けがこんなに綺麗だなんて知らなかった。', "I didn't know sunsets could be this beautiful."],
  [17.0, 20.5, '約束だよ。絶対に忘れないでね。', "It's a promise. Don't you ever forget."],
  [21.0, 25.0, 'カメラを持ってきたから、写真を撮ろう！', "I brought a camera, so let's take pictures!"],
  [26.0, 29.5, 'ありがとう。また会おうね。', "Thank you. Let's meet again."],
];

const ts = (s, sep) => {
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = s % 60;
  const whole = Math.floor(sec), frac = Math.round((sec - whole) * (sep === ',' ? 1000 : 100));
  const pad = (n, w = 2) => String(n).padStart(w, '0');
  return sep === ','
    ? `${pad(h)}:${pad(m)}:${pad(whole)},${pad(frac, 3)}`
    : `${h}:${pad(m)}:${pad(whole)}.${pad(frac)}`;
};

const ass = [
  '[Script Info]', 'ScriptType: v4.00+', 'PlayResX: 1280', 'PlayResY: 720', '',
  '[V4+ Styles]',
  'Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding',
  'Style: Default,Noto Sans JP,52,&H00FFFFFF,&H000000FF,&H00000000,&H64000000,-1,0,0,0,100,100,0,0,1,3,1,2,40,40,40,1',
  'Style: Sign,Noto Sans JP,40,&H00FFE0B0,&H000000FF,&H00402010,&H00000000,0,0,0,0,100,100,0,0,1,2,0,8,40,40,40,1',
  '',
  '[Events]',
  'Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text',
  `Dialogue: 0,${ts(0.5, '.')},${ts(6, '.')},Sign,,0,0,0,,{\\pos(640,80)}第七話「夕焼けの旅」`,
  ...LINES.map(([a, b, jp]) => `Dialogue: 0,${ts(a, '.')},${ts(b, '.')},Default,,0,0,0,,${jp}`),
  '',
].join('\n');
const srt = LINES.map(([a, b, , en], i) => `${i + 1}\n${ts(a, ',')} --> ${ts(b, ',')}\n${en}\n`).join('\n');
writeFileSync(join(OUT, 'jp.ass'), ass);
writeFileSync(join(OUT, 'en.srt'), srt);

function run(name, args) {
  const r = spawnSync(ffmpeg, ['-hide_banner', '-loglevel', 'error', '-y', ...args, join(OUT, name)], {
    encoding: 'utf8',
  });
  if (r.status !== 0) {
    console.error(`FAILED ${name}\n${r.stderr}`);
    process.exit(1);
  }
  console.log(`made samples/${name}`);
}

const video = ['-f', 'lavfi', '-i', 'testsrc2=size=1280x720:rate=24000/1001:duration=30'];
const audio = ['-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000:duration=30'];
const subs = ['-i', join(OUT, 'jp.ass'), '-i', join(OUT, 'en.srt')];
const subMeta = [
  '-map', '0:v', '-map', '1:a', '-map', '2:s', '-map', '3:s',
  '-metadata:s:a:0', 'language=jpn',
  '-metadata:s:s:0', 'language=jpn', '-metadata:s:s:0', 'title=Japanese (ASS)',
  '-metadata:s:s:1', 'language=eng', '-metadata:s:s:1', 'title=English',
];

run('hi10p-ass.mkv', [...video, ...audio, ...subs, ...subMeta,
  '-c:v', 'libx264', '-preset', 'veryfast', '-pix_fmt', 'yuv420p10le', '-profile:v', 'high10',
  '-c:a', 'flac', '-c:s', 'copy']);
run('hevc10.mkv', [...video, ...audio, ...subs, ...subMeta,
  '-c:v', 'libx265', '-preset', 'fast', '-pix_fmt', 'yuv420p10le', '-x265-params', 'log-level=error',
  '-c:a', 'libopus', '-b:a', '96k', '-c:s', 'copy']);
run('h264.mp4', [...video, ...audio, '-map', '0:v', '-map', '1:a',
  '-c:v', 'libx264', '-preset', 'veryfast', '-pix_fmt', 'yuv420p', '-c:a', 'aac']);
