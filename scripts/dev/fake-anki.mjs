// Fake AnkiConnect on 127.0.0.1:8765 for trying the mining flow without Anki; logs requests to artifacts/probe/fake-anki.jsonl. Dev tool.
//
// usage: node scripts/dev/fake-anki.mjs   (Ctrl+C to stop). Answers like AnkiConnect v6 for the actions the app uses.
import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const LOG = join(ROOT, 'artifacts', 'probe', 'fake-anki.jsonl');
mkdirSync(dirname(LOG), { recursive: true });
writeFileSync(LOG, '');

const models = new Set(['Basic']);
const decks = new Set(['Default', 'Japanese::Anime']);
let nextId = 1_700_000_000_000;

function answer(action, p) {
  switch (action) {
    case 'version': return 6;
    case 'deckNames': return [...decks];
    case 'modelNames': return [...models];
    case 'createModel': models.add(p.modelName); return { id: 1 };
    case 'modelFieldNames': return [];
    case 'createDeck': decks.add(p.deck); return 1;
    case 'findNotes': return [];
    case 'storeMediaFile': return p.filename;
    case 'addNote': return ++nextId;
    case 'guiBrowse': return [];
    default: throw new Error(`unsupported action: ${action}`);
  }
}

createServer((req, res) => {
  let body = '';
  req.on('data', (c) => { body += c; });
  req.on('end', () => {
    let out;
    try {
      const r = JSON.parse(body || '{}');
      out = { result: answer(r.action, r.params ?? {}), error: null };
      appendFileSync(LOG, JSON.stringify({ action: r.action, params: r.params }) + '\n');
    } catch (e) {
      out = { result: null, error: String(e.message ?? e) };
    }
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(out));
  });
}).listen(8765, '127.0.0.1', () => console.log('fake AnkiConnect on 127.0.0.1:8765'));
