// Собственные проверки уже записанных пакетов; сетевых зависимостей нет.
const assert=require('node:assert/strict');
function componentText(value){
  if(typeof value==='string'){
    let parsed;try{parsed=JSON.parse(value);}catch{return value;}
    return componentText(parsed);
  }
  if(value?.type==='string'){assert.equal(typeof value.value,'string','Malformed NBT text');return componentText(value.value);}
  if(value?.type==='compound')return componentText(value.value?.text);
  if(typeof value?.text==='string')return value.text;
  assert.fail('Unsupported text component in GUI evidence');
}
function customName(item){
  const components=item.components||[];
  const names=components.filter(component=>component.type==='custom_name');
  assert(names.length<=1,'Duplicate custom name in GUI evidence');
  return names.length?componentText(names[0].data):null;
}
function slotUpdates(rows,windowId){
  assert(Number.isInteger(windowId)&&windowId>0,'Invalid observed window id');
  const updates=[];
  for(const {name,packet} of rows){
    if(name!=='set_slot'&&name!=='window_items')continue;
    if(packet.windowId!==windowId)continue;
    const add=(slot,item)=>{
      assert(Number.isInteger(slot)&&slot>=0,'Invalid GUI slot');
      assert(item&&Number.isInteger(item.itemCount)&&item.itemCount>=0,'Malformed GUI item');
      if(item.itemCount>0){
        assert(Number.isInteger(item.itemId)&&item.itemId>=0,'Malformed GUI item id');
        assert(Array.isArray(item.components),'Malformed GUI components');
        assert.equal(item.addedComponentCount,item.components.length,'GUI component count mismatch');
        customName(item);
      }
      updates.push({slot,item});
    };
    if(name==='set_slot')add(packet.slot,packet.item);
    else{
      assert(Array.isArray(packet.items),'Malformed GUI snapshot');
      for(let slot=0;slot<packet.items.length;slot++)add(slot,packet.items[slot]);
    }
  }
  return updates;
}
function assertObservedWindow(rows,windowId){
  const opens=rows.filter(row=>row.name==='open_window'&&row.packet.windowId===windowId);
  assert.equal(opens.length,1,'Observed GUI open packet missing or duplicated');
  assert.equal(componentText(opens[0].packet.windowTitle),'Observed items','Unexpected observed GUI title');
}
module.exports={slotUpdates,customName,assertObservedWindow};
