#!/usr/bin/env python3
"""Совместимая точка входа проверки запуска26.3 без мастера.

Запускайте в изолированном network namespace, как в migration-checks.yml.
Проверки и формат квитанции находятся в tools/check_startup_263.py.
"""
from pathlib import Path
import runpy

if __name__ == '__main__':
    runpy.run_path(str(Path(__file__).resolve().parents[1] / 'check_startup_263.py'), run_name='__main__')
