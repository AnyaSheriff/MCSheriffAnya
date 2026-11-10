#!/usr/bin/env python3
"""Собственный actual C fault component; fault code не входит в обычный SDK/adapter."""
from pathlib import Path
import tempfile
import shutil
import translate

root=Path(__file__).resolve().parent
translate.bootstrap(toolchain=True)
with tempfile.TemporaryDirectory(prefix='priority-faults-',dir=translate.TARGET) as folder:
 folder=Path(folder);adapter=(root/'adapter.c').read_text()
 anchor='#ifdef MCSA_STAGED_BLOCK_PLACE\n'
 faults=r'''
static int priority_fault;
static volatile uint64_t priority_work;
static void priority_burn_fuel(void) { for(uint64_t i=0;i<1000000;i++)priority_work=priority_work*3+i; }
static void priority_burn_guest(void) {
 struct timespec start,now;clock_gettime(CLOCK_MONOTONIC,&start);
 do {priority_work++;clock_gettime(CLOCK_MONOTONIC,&now);}while((now.tv_sec-start.tv_sec)*1000000000LL+now.tv_nsec-start.tv_nsec<400000000LL);
}
'''
 adapter=adapter.replace('#include <stdint.h>','#include <stdint.h>\n#include <time.h>')
 assert adapter.count(anchor)==1;adapter=adapter.replace(anchor,anchor+faults)
 needle='    int code=dispatch(43);if(code>0)abort();return place_priority_result(code,err);'
 assert adapter.count(needle)==1
 adapter=adapter.replace(needle,r'''
    priority_fault=0;
    if(block->len==15&&memcmp(block->ptr,"minecraft:stone",15)==0)priority_fault=1;
    if(block->len==14&&memcmp(block->ptr,"minecraft:dirt",14)==0)priority_fault=2;
    if(block->len==17&&memcmp(block->ptr,"minecraft:granite",17)==0)priority_fault=3;
    if(block->len==18&&memcmp(block->ptr,"minecraft:andesite",18)==0)priority_fault=4;
    if(block->len==17&&memcmp(block->ptr,"minecraft:diorite",17)==0)priority_fault=5;
    if(block->len==17&&memcmp(block->ptr,"minecraft:calcite",17)==0)priority_fault=6;
    int code=dispatch(43);if(priority_fault==1)abort();if(code>0)abort();return place_priority_result(code,err);
''')
 needle='    clear();place_priority_id(id);integer(priority);integer(current->cancelled);integer(current->can_build);int code=dispatch(44);'
 assert adapter.count(needle)==1
 adapter=adapter.replace(needle,r'''
    if(priority_fault==2&&priority==2)abort();
    if(priority_fault==4)priority_burn_fuel();
    if(priority_fault==5)priority_burn_guest();
    if(priority_fault==6){plugin_string_t name=str("native-budget-proof.txt");plugin_list_u8_t data={0};plugin_string_t error={0};if(!mcsa_api_storage_write(&name,&data,&error))abort();}
''' +needle)
 needle='    clear();place_priority_id(id);integer(final_state->cancelled);integer(final_state->can_build);int code=dispatch(45);if(code>0)abort();return place_priority_result(code,err);'
 assert adapter.count(needle)==1;adapter=adapter.replace(needle,'    if(priority_fault==3)abort();\n'+needle)
 path=folder/'fault-adapter.c';path.write_text(adapter)
 translate.translate(root/'target/place-staged-empty.jar',folder/'output',staged_block_place=True,adapter=path)
 out=root.parents[1]/'tmp/place-staged/plugins/java-place-staged-fault';assert not out.exists()
 shutil.copytree(folder/'output/java-place-staged-empty',out)
 manifest=out/'plugin.toml';manifest.write_text(manifest.read_text().replace('java-place-staged-empty','java-place-staged-fault'))
 evidence=root.parents[1]/'place-staged-evidence/fault-adapter.c';evidence.write_text(adapter)
