#!/usr/bin/env python3
"""Таблицы769 из закреплённых JSON; отдельный Play checklist."""
import sys
from generate_payload_772 import main
if __name__ == '__main__':
    sys.argv.extend(['--client','769'])
    main()
