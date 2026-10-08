// Краткая выписка записи: node sum.js файл [фильтр-regexp]
'use strict';
const fs = require('fs');
const j = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const re = process.argv[3] ? new RegExp(process.argv[3]) : null;
const first = j.window ? j.window.first : 0;
for (const e of j.events) {
  const { tick, ms, phase, ...rest } = e;
  const line = `${tick - first} ${JSON.stringify(rest)}`;
  if (!re || re.test(line)) console.log(line);
}
