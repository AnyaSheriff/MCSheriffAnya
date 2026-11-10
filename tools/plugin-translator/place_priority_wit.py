"""Отдельный optional Place world; старые пакеты не изменяются."""
from pathlib import Path
import shutil
from priority_wit import ROOT

def prepare(destination):
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=False)
    shutil.copyfile(ROOT / 'wit/place-priority-events/world.wit', destination / 'world.wit')
    for name, source in [('api', 'wit/mcsa-api.wit'), ('priority-events', 'wit/priority-events/world.wit')]:
        dependency = destination / 'deps' / name
        dependency.mkdir(parents=True)
        shutil.copyfile(ROOT / source, dependency / 'world.wit')
    return destination
