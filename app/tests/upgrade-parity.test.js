'use strict';
// Prueft, dass die Bewertungslogik im Frontend-Mock (mock.js) exakt der des
// Rust-Backends entspricht — beide werden gegen dieselben Golden-Vectors getestet
// (shared/test-vectors/upgrade-cases.json, siehe golden_tests.rs auf Rust-Seite).
const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const { evaluate } = require('../src/mock.js');
const vectors = JSON.parse(
  fs.readFileSync(path.join(__dirname, '../../shared/test-vectors/upgrade-cases.json'), 'utf8')
);

test('mock.js evaluate() entspricht den geteilten Golden-Vectors', () => {
  assert.ok(vectors.cases.length > 0, 'Golden-Vectors enthalten keine Faelle');
  for (const c of vectors.cases) {
    const ev = evaluate(vectors.thresholds, c.facts);
    assert.strictEqual(ev.status, c.status, 'Status fuer Fall ' + c.name);
    assert.deepStrictEqual(ev.reasons, c.reasons, 'Begruendungen fuer Fall ' + c.name);
  }
});
