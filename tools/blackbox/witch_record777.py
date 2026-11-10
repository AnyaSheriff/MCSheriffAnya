#!/usr/bin/env python3
"""Собственный 777 recorder: дополнительно сохраняем режим/здоровье до записи."""
import sys
import record777
_original_observe = record777.Recorder.observe_packet
_original_record = record777.record

def observe(self, name, payload):
    if not self.recording and name in ('game_state_change', 'update_health', 'entity_effect', 'remove_entity_effect', 'position', 'player_info', 'login'):
        self.preroll.append({'at':record777.time.monotonic(), 'name':name, 'hex':payload.hex()})
    return _original_observe(self, name, payload)

def record(*args, **kwargs):
    values = list(args)
    # Каталог звуков не нужен: проверяем только реальные spawn/effect/damage.
    if len(values) > 6:
        values[6] = None
    else:
        kwargs['sound_output'] = None
    return _original_record(*values, **kwargs)

record777.Recorder.observe_packet = observe
record777.record = record
if __name__ == '__main__':
    if not any(arg.startswith('--max-seconds') for arg in sys.argv):
        raise SystemExit('explicit --max-seconds required')
    record777.main()
