// Generates a 24-minute episode-like stress sample (1080p HEVC 10-bit, ~400 fansub-style ASS lines with an OP in karaoke, signs, overlaps, \N, plus English SRT) in samples/episode/.
//
// usage: node scripts/make-episode.mjs <full ffmpeg with libx265> [minutes=24]
// Deterministic (seeded), local test data only (samples/ is gitignored); never shipped.
import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const OUT = join(ROOT, 'samples', 'episode');
const ffmpeg = process.argv[2];
const minutes = Number(process.argv[3] ?? 24);
if (!ffmpeg) {
  console.error('usage: node scripts/make-episode.mjs <ffmpeg.exe with libx265> [minutes]');
  process.exit(2);
}
mkdirSync(OUT, { recursive: true });

// Seeded generator so the file is the same every time.
let seed = 20261108;
const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
const pick = (a) => a[Math.floor(rnd() * a.length)];

/** [Japanese, English] dialogue the generator draws from (everyday anime phrasing, conjugations). */
const LINES = [
  ['もう諦めないって決めたんだ。', "I decided I'm not giving up anymore."],
  ['食べさせられなかったのは残念だね。', "It's a shame we couldn't make them eat it."],
  ['明日、一緒に行こうよ。', "Let's go together tomorrow."],
  ['夕焼けがこんなに綺麗だなんて知らなかった。', "I didn't know sunsets could be this beautiful."],
  ['約束だよ。絶対に忘れないでね。', "It's a promise. Don't you ever forget."],
  ['カメラを持ってきたから、写真を撮ろう！', "I brought a camera, so let's take pictures!"],
  ['ありがとう。また会おうね。', "Thank you. Let's meet again."],
  ['ちょっと待って！まだ話は終わってない。', "Wait a second! I'm not done talking."],
  ['お前、本当にそれでいいのか？', 'Are you really okay with that?'],
  ['大丈夫、心配しないで。', "It's fine, don't worry."],
  ['どうしてそんなことを言うの？', 'Why would you say something like that?'],
  ['俺たちは負けるわけにはいかないんだ。', "We can't afford to lose."],
  ['先生に聞いてみたら？', 'Why not ask the teacher?'],
  ['今日は雨が降りそうだから、傘を持っていこう。', "It looks like rain today, so let's take an umbrella."],
  ['あの時、助けてくれてありがとう。', 'Thanks for helping me back then.'],
  ['何があっても、君を守るから。', "Whatever happens, I'll protect you."],
  ['すみません、道に迷ってしまいました。', "Excuse me, I've gotten lost."],
  ['この本、読んだことある？', 'Have you read this book?'],
  ['早く行かないと遅刻しちゃうよ！', "If we don't hurry we'll be late!"],
  ['信じられない…こんなことって。', "I can't believe it... something like this."],
  ['お腹すいた。何か食べに行かない？', "I'm hungry. Want to go eat something?"],
  ['夢を叶えるために、ここまで来たんだ。', 'I came all this way to make my dream come true.'],
  ['もし僕が魔法を使えたら、世界を変えられるのに。', 'If I could use magic, I could change the world.'],
  ['静かにしてください。図書館ですよ。', "Please be quiet. This is a library."],
  ['そんなに怒らなくてもいいじゃない。', "You don't have to get so angry."],
  ['覚えていますか？初めて会った日のこと。', 'Do you remember the day we first met?'],
  ['勉強しなければならないのに、眠くて仕方がない。', "I have to study, but I can't help being sleepy."],
  ['行ってきます！', "I'm heading out!"],
  ['おかえりなさい。', 'Welcome home.'],
  ['すっげー！見て見て！', 'Awesome! Look, look!'],
];
const OP = ['きらめく ゆめを おいかけて', 'とおい そらへ はばたこう', 'だれも しらない あしたへ'];
const SIGNS = ['第七話「夕焼けの旅」', '放課後', '三年後', '図書室', '駅前'];

const ts = (ms, sep) => {
  const h = Math.floor(ms / 3600000), m = Math.floor(ms / 60000) % 60, s = Math.floor(ms / 1000) % 60;
  const p = (n, w = 2) => String(n).padStart(w, '0');
  return sep === ','
    ? `${p(h)}:${p(m)}:${p(s)},${p(ms % 1000, 3)}`
    : `${h}:${p(m)}:${p(s)}.${p(Math.floor((ms % 1000) / 10))}`;
};

const total = minutes * 60_000;
const events = []; // [start, end, style, text(ass), en|null]
// OP 1:30-3:00 with karaoke, then dialogue with gaps, two-speaker overlaps, line breaks, signs.
const opStart = 90_000;
OP.forEach((line, i) => {
  const st = opStart + i * 25_000;
  const k = line.split(' ').map((w) => `{\\k${40 + Math.floor(rnd() * 40)}}${w}`).join(' ');
  events.push([st, st + 20_000, 'OP', k, null]);
});
let t = 5_000;
let n = 0;
while (t < total - 8_000) {
  if (t >= opStart - 2_000 && t < opStart + 80_000) { t = opStart + 80_000; continue; }
  const [jp, en] = pick(LINES);
  const len = 1_500 + Math.floor(rnd() * 3_000);
  const r = rnd();
  if (r < 0.08) {
    // Two speakers at once: second line overlaps on top (\an8), common in fansubs.
    const [jp2, en2] = pick(LINES);
    events.push([t, t + len, 'Default', jp, en]);
    events.push([t + 300, t + len + 300, 'Top', `{\\an8}${jp2}`, en2]);
  } else if (r < 0.15) {
    // Long line split with \N.
    const cut = Math.ceil(jp.length / 2);
    events.push([t, t + len + 1_000, 'Default', `${jp.slice(0, cut)}\\N${jp.slice(cut)}`, en]);
  } else if (r < 0.2) {
    events.push([t, t + len, 'Italics', `{\\i1}${jp}{\\i0}`, en]);
  } else {
    events.push([t, t + len, 'Default', jp, en]);
  }
  if (rnd() < 0.05) events.push([t, t + 4_000, 'Sign', `{\\pos(960,120)\\fad(200,200)}${pick(SIGNS)}`, null]);
  t += len + 300 + Math.floor(rnd() * 2_500);
  n++;
}
events.sort((a, b) => a[0] - b[0]);

const ass = [
  '[Script Info]', 'ScriptType: v4.00+', 'PlayResX: 1920', 'PlayResY: 1080', 'WrapStyle: 0', '',
  '[V4+ Styles]',
  'Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding',
  'Style: Default,Noto Sans JP,72,&H00FFFFFF,&H000000FF,&H00000000,&H64000000,-1,0,0,0,100,100,0,0,1,4,1,2,60,60,60,1',
  'Style: Italics,Noto Sans JP,72,&H00FFFFFF,&H000000FF,&H00000000,&H64000000,-1,-1,0,0,100,100,0,0,1,4,1,2,60,60,60,1',
  'Style: Top,Noto Sans JP,72,&H00FFFFFF,&H000000FF,&H00000000,&H64000000,-1,0,0,0,100,100,0,0,1,4,1,8,60,60,60,1',
  'Style: Sign,Noto Sans JP,60,&H00FFE0B0,&H000000FF,&H00402010,&H00000000,0,0,0,0,100,100,0,0,1,3,0,8,60,60,60,1',
  'Style: OP,Noto Sans JP,56,&H00FFFFFF,&H00FF7AA2,&H00301020,&H00000000,-1,0,0,0,100,100,0,0,1,3,0,8,60,60,40,1',
  '',
  '[Events]',
  'Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text',
  'Comment: 0,0:00:00.00,0:00:00.00,Default,,0,0,0,,stress sample generated by scripts/make-episode.mjs',
  ...events.map(([s, e, st, tx]) => `Dialogue: 0,${ts(s, '.')},${ts(e, '.')},${st},,0,0,0,,${tx}`),
  '',
].join('\n');
const srt = events
  .filter((e) => e[4])
  .map(([s, e, , , en], i) => `${i + 1}\n${ts(s, ',')} --> ${ts(e, ',')}\n${en}\n`)
  .join('\n');
writeFileSync(join(OUT, 'episode.ja.ass'), ass);
writeFileSync(join(OUT, 'episode.en.srt'), srt);
console.log(`subtitles: ${events.length} events (${n} dialogue steps), ${minutes} min`);

const args = [
  '-hide_banner', '-loglevel', 'error', '-y',
  '-f', 'lavfi', '-i', `testsrc2=size=1920x1080:rate=24000/1001:duration=${minutes * 60}`,
  '-f', 'lavfi', '-i', `sine=frequency=440:sample_rate=48000:duration=${minutes * 60}`,
  '-i', join(OUT, 'episode.ja.ass'), '-i', join(OUT, 'episode.en.srt'),
  '-map', '0:v', '-map', '1:a', '-map', '2:s', '-map', '3:s',
  '-metadata:s:a:0', 'language=jpn', '-metadata:s:s:0', 'language=jpn', '-metadata:s:s:1', 'language=eng',
  '-c:v', 'libx265', '-preset', 'ultrafast', '-crf', '34', '-pix_fmt', 'yuv420p10le', '-x265-params', 'log-level=error',
  '-c:a', 'libopus', '-b:a', '64k', '-c:s', 'copy',
  join(OUT, `[Stress] Sunset Journey - 07 (1080p HEVC 10bit).mkv`),
];
const t0 = Date.now();
const r = spawnSync(ffmpeg, args, { encoding: 'utf8' });
if (r.status !== 0) {
  console.error(r.stderr);
  process.exit(1);
}
console.log(`video: done in ${Math.round((Date.now() - t0) / 1000)} s`);
