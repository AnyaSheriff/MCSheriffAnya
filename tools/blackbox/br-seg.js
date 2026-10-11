// Печатает участок нормализованной записи между метками: node br-seg.js <сценарий> <метка> [ширина] [bds|ours]
const fs = require('fs'), path = require('path');
const [name, tag, w = '300', only] = process.argv.slice(2);
for (const t of ['bds', 'ours']) {
  if (only && only !== t) continue;
  const f = path.join(__dirname, '..', '..', 'tmp', 'br', 'norm', `${name}.${t}.txt`);
  const lines = fs.readFileSync(f, 'utf8').split('\n');
  let on = false; console.log('=== ' + t);
  for (const l of lines) {
    if (l.startsWith('## TAG ')) on = l === '## TAG ' + tag;
    if (on) console.log(l.slice(0, +w));
  }
}
