// node v-wsum.js win.json — короткая сводка окон (что пришло после клика по блоку)
const path = require('path');
const f = path.resolve(process.argv[2]);
for (const r of require(f)) {
  const s = r.pk.filter((p) => p.n !== 'network_chunk_publisher_update' && p.n !== 'move_player' && p.n !== 'set_time' && p.n !== 'move_entity' && p.n !== 'move_actor_delta').map((p) => {
    if (p.n === 'container_open') return `OPEN(id=${p.p.window_id},type=${p.p.window_type},pos=${JSON.stringify(p.p.coordinates)})`;
    if (p.n === 'inventory_content') return `CONTENT(w=${p.p.window_id},n=${p.p.input && p.p.input.length})`;
    if (p.n === 'container_set_data') return `DATA(w=${p.p.window_id},${p.p.property}=${p.p.value})`;
    if (p.n === 'inventory_slot') return `SLOT(w=${p.p.window_id},${p.p.slot})`;
    if (p.n === 'container_close') return `CLOSE(${p.p.window_id})`;
    if (p.n === 'text') return `TEXT(${p.p.message})`;
    return p.n;
  });
  console.log(r.block.padEnd(18), s.join(' '));
}
