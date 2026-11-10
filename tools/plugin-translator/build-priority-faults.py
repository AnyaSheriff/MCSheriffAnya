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
 anchor='#ifdef MCSA_STAGED_BLOCK_BREAK\n'
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
 needle='    return priority_result(dispatch(40),err);'
 assert adapter.count(needle)==1
 adapter=adapter.replace(needle,r'''
    priority_fault=0;
    if(block->len==10&&memcmp(block->ptr,"test:begin",10)==0)priority_fault=1;
    if(block->len==10&&memcmp(block->ptr,"test:phase",10)==0)priority_fault=2;
    if(block->len==11&&memcmp(block->ptr,"test:finish",11)==0)priority_fault=3;
    if(block->len==9&&memcmp(block->ptr,"test:fuel",9)==0)priority_fault=4;
    if(block->len==10&&memcmp(block->ptr,"test:guest",10)==0)priority_fault=5;
    if(block->len==11&&memcmp(block->ptr,"test:native",11)==0)priority_fault=6;
    int code=dispatch(40);if(priority_fault==1)abort();return priority_result(code,err);
''')
 needle='    clear();priority_id(id);integer(priority);integer(cancelled);int code=dispatch(41);'
 assert adapter.count(needle)==1
 adapter=adapter.replace(needle,r'''
    if(priority_fault==2&&priority==2)abort();
    if(priority_fault==4)priority_burn_fuel();
    if(priority_fault==5)priority_burn_guest();
    if(priority_fault==6){plugin_string_t name=str("native-budget-proof.txt");plugin_list_u8_t data={0};plugin_string_t error={0};if(!mcsa_api_storage_write(&name,&data,&error))abort();}
''' +needle)
 needle='    clear();priority_id(id);integer(cancelled);return priority_result(dispatch(42),err);'
 assert adapter.count(needle)==1;adapter=adapter.replace(needle,'    if(priority_fault==3)abort();\n'+needle)
 path=folder/'fault-adapter.c';path.write_text(adapter)
 translate.translate(root/'target/priority-empty.jar',folder/'output',staged_block_break=True,adapter=path)
 out=root.parents[1]/'tmp/java-priority-staged/plugins/java-priority-fault';assert not out.exists()
 shutil.copytree(folder/'output/java-priority-empty',out)
 manifest=out/'plugin.toml';manifest.write_text(manifest.read_text().replace('java-priority-empty','java-priority-fault'))
 evidence=root.parents[1]/'staged-evidence/fault-adapter.c';evidence.write_text(adapter)
