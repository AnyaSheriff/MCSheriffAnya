#!/usr/bin/env python3
"""Таблицы771 через общий генератор772; отдельная проверка равных типов JSON."""
import sys
from generate_payload_772 import main
if __name__=='__main__':
    sys.argv.extend(['--client','771'])
    main()
