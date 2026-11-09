// Generates a test episode with REAL speech at known times, for audio-based subtitle sync (samples/speech/).
//
// usage: node scripts/make-speech-sample.mjs [minutes=6]
// Speech comes from the Windows voices (System.Speech; English is fine, voice detection doesn't depend on
// the language). Lines alternate between a male and a female voice. Background music (chords, drums
// and a sung-like melody) plays under the dialogue, louder in a music-only "OP" and "ED", and there is
// a 20 s "ad break". The Japanese subtitles are written at the true times (start 100 ms before the
// speech, end 300 ms after, like real subs) and also deliberately out of sync:
//   samples/speech/exact/Ep.mkv + Ep.ja.srt   already in sync (sync must leave it alone)
//   samples/speech/const/Ep.mkv + Ep.ja.srt   every line 2.7 s late
//   samples/speech/split/Ep.mkv + Ep.ja.srt   1.2 s early before the ad break, 3.4 s late after it
//   (each folder also has Ep.en.srt, off by a DIFFERENT amount: 1.3 s early / +0.6 s then 2 s early)
//   samples/speech/truth.json                 the true start/end of every line (ms)
// Deterministic apart from the TTS engine, local test data only (samples/ is gitignored); never shipped.
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const OUT = join(ROOT, 'samples', 'speech');
const TTS = join(OUT, 'tts');
const FFMPEG = join(ROOT, 'src-tauri', 'lib', 'ffmpeg.exe');
const minutes = Number(process.argv[2] ?? 6);
const RATE = 16000;
// Music level under dialogue (env BED, default 0.07); 0.14 is loud BGM, about -11 dB below the voices.
const BED = Number(process.env.BED ?? 0.07);
mkdirSync(TTS, { recursive: true });

let seed = 20261109;
const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);

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
  ['はい。', 'Yes.'],
  ['えっ、本当？', 'Huh, really?'],
  ['行くぞ！', "Let's go!"],
  ['そんなの、ずるいよ。', "That's not fair."],
];

// ---- timeline -----------------------------------------------------------------------------------
const OP_END = 80_000; // music-only opening (with "sung" melody)
const total = minutes * 60_000;
const ED_START = total - 60_000;
const AD_AT = Math.round((OP_END + ED_START) / 2); // 20 s ad break (quiet music, no speech)
const AD_LEN = 20_000;

// ---- speech: synthesize every distinct line once per voice (one PowerShell run) -----------------
const VOICES = ['Microsoft David Desktop', 'Microsoft Zira Desktop'];
const jobs = [];
LINES.forEach(([, en], i) => VOICES.forEach((v, vi) => jobs.push({ text: en, voice: v, file: join(TTS, `l${i}-v${vi}.wav`) })));
const jobsFile = join(TTS, 'jobs.json');
writeFileSync(jobsFile, JSON.stringify(jobs));
const ps = `
Add-Type -AssemblyName System.Speech
$jobs = Get-Content -Raw -Encoding UTF8 '${jobsFile}' | ConvertFrom-Json
$fmt = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(${RATE}, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono)
foreach ($j in $jobs) {
  $s = New-Object System.Speech.Synthesis.SpeechSynthesizer
  $s.SelectVoice($j.voice)
  $s.SetOutputToWaveFile($j.file, $fmt)
  $s.Speak($j.text)
  $s.Dispose()
}`;
const r = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', ps], { encoding: 'utf8' });
if (r.status !== 0) {
  console.error('speech synthesis failed:', r.stderr);
  process.exit(1);
}

/** 16-bit mono PCM samples of a wav written by System.Speech, with leading/trailing silence trimmed. */
function readWav(file) {
  const b = readFileSync(file);
  let p = 12;
  while (p < b.length - 8 && b.toString('ascii', p, p + 4) !== 'data') p += 8 + b.readUInt32LE(p + 4);
  const n = b.readUInt32LE(p + 4) / 2;
  const s = new Int16Array(n);
  for (let i = 0; i < n; i++) s[i] = b.readInt16LE(p + 8 + i * 2);
  let a = 0;
  let e = n - 1;
  while (a < n && Math.abs(s[a]) < 300) a++;
  while (e > a && Math.abs(s[e]) < 300) e--;
  return s.slice(a, e + 1);
}

// ---- place lines ---------------------------------------------------------------------------------
const lines = []; // { start, end, jp, en, pcm }
let t = OP_END + 2_000;
let k = 0;
while (t < ED_START - 5_000) {
  if (t >= AD_AT && t < AD_AT + AD_LEN) {
    t = AD_AT + AD_LEN + 1_500;
    continue;
  }
  const li = Math.floor(rnd() * LINES.length);
  const pcm = readWav(join(TTS, `l${li}-v${k % 2}.wav`));
  const len = Math.round((pcm.length / RATE) * 1000);
  if (t < AD_AT && t + len > AD_AT) {
    t = AD_AT + AD_LEN + 1_500;
    continue;
  }
  lines.push({ start: t, end: t + len, jp: LINES[li][0], en: LINES[li][1], pcm });
  t += len + 300 + Math.round(rnd() * (rnd() < 0.2 ? 6_000 : 2_200));
  k++;
}

// ---- mix: music bed + speech ----------------------------------------------------------------------
const N = Math.round((total / 1000) * RATE);
const mix = new Float32Array(N);
const CHORDS = [[220, 277.2, 329.6], [196, 246.9, 293.7], [174.6, 220, 261.6], [196, 246.9, 311.1]];
const MELODY = [440, 494, 523, 587, 659, 587, 523, 494];
let noise = 0;
for (let i = 0; i < N; i++) {
  const ts = i / RATE;
  const ms = ts * 1000;
  const loud = ms < OP_END || ms >= ED_START;
  const inAd = ms >= AD_AT && ms < AD_AT + AD_LEN;
  const bed = loud ? 0.22 : inAd ? 0.05 : BED; // default 0.07: under dialogue the music is ~-17 dB below the voices
  const chord = CHORDS[Math.floor(ts / 2) % CHORDS.length];
  let v = chord.reduce((acc, f) => acc + Math.sin(2 * Math.PI * f * ts), 0) / 3;
  // Drum: a decaying noise burst every 0.5 s.
  noise = noise * 0.97 + (Math.random() * 2 - 1) * 0.03;
  const beat = ts % 0.5;
  v += noise * 6 * Math.exp(-beat * 18);
  // Sung-like melody with vibrato in the OP/ED (looks most like a voice to a detector).
  if (loud) {
    const f = MELODY[Math.floor(ts / 0.6) % MELODY.length] * (1 + 0.01 * Math.sin(2 * Math.PI * 5.5 * ts));
    v += 1.4 * Math.sin(2 * Math.PI * f * ts) + 0.5 * Math.sin(4 * Math.PI * f * ts);
  }
  mix[i] = v * bed;
}
for (const l of lines) {
  const o = Math.round((l.start / 1000) * RATE);
  for (let j = 0; j < l.pcm.length && o + j < N; j++) mix[o + j] += (l.pcm[j] / 32768) * 0.8;
}
const wav = Buffer.alloc(44 + N * 2);
wav.write('RIFF', 0);
wav.writeUInt32LE(36 + N * 2, 4);
wav.write('WAVEfmt ', 8);
wav.writeUInt32LE(16, 16);
wav.writeUInt16LE(1, 20);
wav.writeUInt16LE(1, 22);
wav.writeUInt32LE(RATE, 24);
wav.writeUInt32LE(RATE * 2, 28);
wav.writeUInt16LE(2, 32);
wav.writeUInt16LE(16, 34);
wav.write('data', 36);
wav.writeUInt32LE(N * 2, 40);
for (let i = 0; i < N; i++) wav.writeInt16LE(Math.max(-32768, Math.min(32767, Math.round(mix[i] * 32767))), 44 + i * 2);
const wavFile = join(TTS, 'mix.wav');
writeFileSync(wavFile, wav);

const mkv = join(OUT, 'Ep.mkv');
const f = spawnSync(FFMPEG, ['-v', 'error', '-y', '-f', 'lavfi', '-i', `color=c=0x1d2130:s=320x180:r=10:d=${total / 1000}`,
  '-i', wavFile, '-map', '0:v', '-map', '1:a', '-c:v', 'mpeg4', '-q:v', '10', '-c:a', 'aac', '-b:a', '96k',
  '-metadata:s:a:0', 'language=jpn', '-shortest', mkv], { encoding: 'utf8' });
if (f.status !== 0) {
  console.error('ffmpeg failed:', f.stderr);
  process.exit(1);
}

// ---- subtitles -------------------------------------------------------------------------------------
const ts = (ms) => {
  ms = Math.max(0, ms);
  const p = (n, w = 2) => String(n).padStart(w, '0');
  return `${p(Math.floor(ms / 3_600_000))}:${p(Math.floor(ms / 60_000) % 60)}:${p(Math.floor(ms / 1000) % 60)},${p(ms % 1000, 3)}`;
};
const truth = lines.map((l) => ({ start: l.start - 100, end: l.end + 300, jp: l.jp }));
const srt = (shift) => truth.map((c, i) => `${i + 1}\n${ts(c.start + shift(c))} --> ${ts(c.end + shift(c))}\n${c.jp}\n`).join('\n');
writeFileSync(join(OUT, 'truth.json'), JSON.stringify({ lines: truth, adAt: AD_AT, adLen: AD_LEN }, null, 1));
writeFileSync(join(OUT, 'Ep.en.srt'), truth.map((c, i) => `${i + 1}\n${ts(c.start)} --> ${ts(c.end)}\n${lines[i].en}\n`).join('\n'));
// English from yet another release, off by its own amount (it must follow the synced Japanese).
const enSrt = (shift) => truth.map((c, i) => `${i + 1}\n${ts(c.start + shift(c))} --> ${ts(c.end + shift(c))}\n${lines[i].en}\n`).join('\n');
for (const [dir, shift, enShift] of [
  ['exact', () => 0, () => 0],
  ['const', () => 2_700, () => -1_300],
  ['split', (c) => (c.start < AD_AT ? -1_200 : 3_400), (c) => (c.start < AD_AT ? 600 : -2_000)],
]) {
  mkdirSync(join(OUT, dir), { recursive: true });
  copyFileSync(mkv, join(OUT, dir, 'Ep.mkv'));
  writeFileSync(join(OUT, dir, 'Ep.ja.srt'), srt(shift));
  writeFileSync(join(OUT, dir, 'Ep.en.srt'), enSrt(enShift));
}
// messy/: like a real fansub. 6.5 s late, the OP lyrics as lines (sung, not spoken), sign lines with
// nobody speaking, and three dialogue lines missing. expect.json: each cue's true start, or null when
// it isn't dialogue that can be checked.
{
  const cues = [];
  for (let i = 0; i < 6; i++) cues.push({ start: 8_000 + i * 11_000, end: 8_000 + i * 11_000 + 6_000, text: `♪ 夕焼けの空に ${i + 1}`, truth: null });
  truth.forEach((c, i) => {
    if (i === 5 || i === 17 || i === 31) return; // untranslated lines
    cues.push({ start: c.start, end: c.end, text: c.jp, truth: c.start });
    if (i % 9 === 4) cues.push({ start: c.end + 400, end: c.end + 2_400, text: '【第七話　夕焼けの旅】', truth: null }); // a sign
  });
  cues.sort((a, b) => a.start - b.start);
  const late = 6_500;
  mkdirSync(join(OUT, 'messy'), { recursive: true });
  copyFileSync(mkv, join(OUT, 'messy', 'Ep.mkv'));
  writeFileSync(join(OUT, 'messy', 'Ep.ja.srt'), cues.map((c, i) => `${i + 1}\n${ts(c.start + late)} --> ${ts(c.end + late)}\n${c.text}\n`).join('\n'));
  writeFileSync(join(OUT, 'messy', 'Ep.en.srt'), enSrt(() => 0));
  writeFileSync(join(OUT, 'messy', 'expect.json'), JSON.stringify(cues.map((c) => c.truth)));
}
console.log(`samples/speech: ${lines.length} lines over ${minutes} min (OP to ${OP_END / 1000} s, ad break at ${AD_AT / 1000} s, ED from ${ED_START / 1000} s)`);
