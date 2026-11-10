"""Подготовить отдельный пакет без копии обязательного API в Git."""
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[2]

def prepare(destination):
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=False)
    shutil.copyfile(ROOT / 'wit/priority-events/world.wit', destination / 'world.wit')
    dependency = destination / 'deps/api'
    dependency.mkdir(parents=True)
    shutil.copyfile(ROOT / 'wit/mcsa-api.wit', dependency / 'api.wit')
    return destination
