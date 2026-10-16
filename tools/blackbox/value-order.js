// Порядок записей карт не является значением. Записи armorstand-equip-break.md:16–21
// и grindstone.md:6–14: те же атрибуты/зачарования приходят в другом порядке.
// Только известные списки; значения, повторы и порядок прочих массивов сохраняются.
'use strict';

function valueOrder(text) {
  return text.replace(/"(enchantments|properties)":(\[(?:[^\[\]]|\[[^\[\]]*\])*\])/g, (whole, field, array) => {
    try {
      const values = JSON.parse(array);
      const key = field === 'enchantments' ? 'id' : 'key';
      if (!values.every(v => v && typeof v === 'object' && v[key] !== undefined)) return whole;
      values.sort((a, b) => String(a[key]).localeCompare(String(b[key])));
      return `"${field}":${JSON.stringify(values)}`;
    } catch { return whole; }
  });
}

module.exports = { valueOrder };
