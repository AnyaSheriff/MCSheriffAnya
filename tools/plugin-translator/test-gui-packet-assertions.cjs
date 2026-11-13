const assert=require('node:assert/strict');
const {slotUpdates,customName,assertObservedWindow}=require('./gui-packet-assertions.cjs');
let checks=0;
const check=fn=>{fn();checks++;};
const item=(itemCount,name)=>({itemCount,itemId:899,addedComponentCount:name?1:0,removedComponentCount:0,components:name?[{type:'custom_name',data:{type:'string',value:name}}]:[],removeComponents:[]});
const sparse=(windowId,slot,value)=>({name:'set_slot',packet:{windowId,slot,item:value}});
const full=(windowId,slot,value)=>{const items=Array.from({length:27},()=>({itemCount:0}));items[slot]=value;return {name:'window_items',packet:{windowId,items}};};
const matches=(rows,count,name)=>slotUpdates(rows,3).some(row=>row.slot===13&&row.item.itemCount===count&&customName(row.item)===name);
for(const make of [sparse,full]){
 check(()=>assert(matches([make(3,13,item(2,'Set item observed'))],2,'Set item observed')));
 check(()=>assert(matches([make(3,13,item(3,'Decoded item observed'))],3,'Decoded item observed')));
 check(()=>assert(!matches([make(2,13,item(2,'Set item observed'))],2,'Set item observed')));
 check(()=>assert(!matches([make(3,12,item(2,'Set item observed'))],2,'Set item observed')));
 check(()=>assert(!matches([make(3,13,item(1,'Set item observed'))],2,'Set item observed')));
 check(()=>assert(!matches([make(3,13,item(2,'Wrong name'))],2,'Set item observed')));
 check(()=>assert(slotUpdates([make(3,14,item(9,'Stale item rejected'))],3).some(row=>customName(row.item)==='Stale item rejected')));
 check(()=>assert.throws(()=>slotUpdates([make(3,13,{itemCount:2})],3)));
 check(()=>{const bad=item(2);bad.addedComponentCount=1;assert.throws(()=>slotUpdates([make(3,13,bad)],3));});
 check(()=>{const before=[make(3,13,item(2,'Set item observed'))];const start=before.length;before.push(make(3,13,item(3,'Decoded item observed')));assert(!matches(before.slice(start),2,'Set item observed'));});
 check(()=>assert(slotUpdates([make(3,15,item(4))],3).some(row=>row.slot===15&&row.item.itemCount===4)));
}
for(const windowTitle of ['{"text":"Observed items"}',{type:'string',value:'Observed items'},{type:'compound',value:{text:{type:'string',value:'Observed items'}}}])check(()=>assertObservedWindow([{name:'open_window',packet:{windowId:3,windowTitle}}],3));
check(()=>assert.throws(()=>assertObservedWindow([{name:'open_window',packet:{windowId:2,windowTitle:'Observed items'}}],3)));
check(()=>assert.throws(()=>assertObservedWindow([{name:'open_window',packet:{windowId:3,windowTitle:'Other'}}],3)));
check(()=>assert.throws(()=>slotUpdates([{name:'window_items',packet:{windowId:3,items:null}}],3)));
console.log(`PASS: ${checks} finite GUI packet assertion checks`);
