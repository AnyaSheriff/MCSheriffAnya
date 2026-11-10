#!/usr/bin/env python3
"""Только дополнительные собственные safety fixtures, baseline не меняется."""
from pathlib import Path
import shutil
from build_guard import run
root=Path(__file__).resolve().parent
for suffix in ['observer','mutator','empty','failed']:
 name='priority-'+suffix;folder=root/'fixtures'/name;target=root/'target/fixtures'/name
 assert not target.exists();target.mkdir(parents=True)
 run(['javac','-proc:none','--release','11','-encoding','UTF-8','-cp',str(root/'target/mcsa-bukkit.jar'),'-d',str(target),*map(str,folder.glob('*.java'))],check=True)
 run(['jar','cf',str(root/'target'/(name+'.jar')),'-C',str(target),'.','-C',str(folder),'plugin.yml'],check=True)
