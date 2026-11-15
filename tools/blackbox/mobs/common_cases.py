"""Управляемые действия; случайные маршруты ИИ здесь не сравниваются."""
try:
    from .catalog import catalog
except ImportError:
    from catalog import catalog

BASE_NBT = 'NoAI:1b,NoGravity:1b,PersistenceRequired:1b,Rotation:[37.0f,0.0f]'


def common_cases():
    cases = []
    for mob in catalog():
        name = mob['mob']
        def add(family, phases, fields, requirements, policy='exact'):
            cases.append(dict(id=f'{name}.{family}', mob=name, family=family,
                              nbt=BASE_NBT, environment=mob['environment'],
                              phases=phases, compare_fields=fields,
                              reference_requirements=requirements, policy=policy))
        def phase(name, commands=(), ticks=2):
            return dict(id=name, commands=list(commands), ticks=ticks)
        add('controlled_spawn', [phase('spawn', ticks=3)],
            ['type', 'position', 'rotation', 'health'], ['spawn_observed'])
        # Проверяем конечное состояние, не задержку команды: два полных
        # трёхтактовых цикла сетевого слежения, одинаково у обеих сторон.
        add('turn', [phase('before'), phase('turned', ['tp {target} {x} {y} {z} 127 0'], ticks=6)],
            ['position', 'rotation'], ['spawn_observed', 'rotation_changed'])
        add('teleport', [phase('before'), phase('moved', ['tp {target} 7.5 103 2.5 37 0'], ticks=6)],
            ['position', 'rotation'], ['spawn_observed', 'position_changed'])
        # Это уход из области слежения, а не смерть. Возврат доказывает сохранение сущности.
        add('tracking_leave_return', [phase('before'),
            phase('outside', ['tp {target} 128.5 101 0.5'], 4),
            phase('returned', ['tp {target} {x} {y} {z} 37 0'], 4)],
            ['type', 'position', 'removed'],
            ['spawn_observed', 'removal_observed', 'respawn_observed'])
        if not mob['generic_damage_exclusion']:
            add('damage', [phase('before'),
                phase('hurt', ['damage {target} 1 minecraft:generic'], 1), phase('settled', ticks=12)],
                ['health', 'damage', 'status'],
                ['spawn_observed', 'health_decreased', 'damage_event_observed'], 'predicate')
            # /kill должен реально убить; отсутствие смерти у эталона делает запись невалидной.
            add('death', [phase('before'), phase('killed', ['kill {target}'], 1),
                          phase('after_death', ticks=30)],
                ['health', 'status', 'removed'],
                ['spawn_observed', 'death_event_observed', 'removal_observed'], 'predicate')
    return cases
