// Какие сущности есть в записях BDS: node br-labels.js <каталог>
const fs = require('fs');
const dir = process.argv[2];

for (const f of fs.readdirSync(dir).filter(x => /^bds-.*\.jsonl$/.test(x))) {
  const seen = [];

  for (const l of fs.readFileSync(dir + '/' + f, 'utf8').split('\n')) {
    if (!l) continue;

    const o = JSON.parse(l);

    if (o.n === 'add_entity') seen.push(o.label + ':' + o.p.entity_type.replace('minecraft:', ''));
  }

  console.log('==', f, seen.length);
  console.log(seen.join(' '));
}
