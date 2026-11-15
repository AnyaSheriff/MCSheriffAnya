"""Проверка статического описания полей перед локальной подписью (без ключей)."""
import json
import re

BASE_KEYS = set('motd server-port bedrock-port max-players difficulty level-seed online-mode offline-license-mode auth-enabled bedrock-online-mode view-distance simulation-distance pause-when-empty-seconds parallel-dimension-ticks dimension-tick-threads auth-mode prevent-proxy-connections autumn-forests pink-cherry-groves portal-delay prefetch-chunks prefetch-at-once network-compression-threshold text-info tab-tps debug send-error-reports bedrock-show-coordinates java-version console-colors'.split())
RESERVED_GROUPS = {'Оформление', 'Резервные копии', 'Обновления', 'Обновления ядра', 'Обновления приложения'}


def _text(value, maximum, empty=False):
    if not isinstance(value, str) or (not empty and not value.strip()) or len(value.encode('utf-16-le')) // 2 > maximum or any(c in value for c in '\r\n\0'):
        raise ValueError('Неверное текстовое поле описания настроек')
    return value


def _integer(value):
    if type(value) is not int or not -2147483648 <= value <= 2147483647:
        raise ValueError('Ожидалось целое 32-битное число')
    return value


def validate_settings(data):
    if not isinstance(data, dict) or _integer(data.get('format')) != 1:
        raise ValueError('Неизвестный формат настроек')
    if len(json.dumps(data, ensure_ascii=False, separators=(',', ':')).encode()) > 35000:
        raise ValueError('Описание настроек превышает 35 КБ')
    fields = data.get('fields')
    if not isinstance(fields, list) or len(fields) > 128:
        raise ValueError('Допускается не более 128 полей')
    keys = set(BASE_KEYS)
    for field in fields:
        if not isinstance(field, dict) or field.get('file') not in ('mcsa', 'server'):
            raise ValueError('Неверный конфиг')
        key = _text(field.get('key'), 80)
        if not re.fullmatch('[a-z][a-z0-9-]{0,79}', key) or key in keys:
            raise ValueError('Неверный или повторный ключ')
        keys.add(key)
        group = _text(field.get('group'), 80)
        if group in RESERVED_GROUPS:
            raise ValueError('Зарезервированный раздел приложения')
        _text(field.get('title'), 160)
        _text(field.get('help'), 2000, True)
        default = _text(field.get('default'), 2048, True)
        kind = field.get('type')
        if kind == 'boolean':
            if default not in ('false', 'true'):
                raise ValueError('Неверное значение переключателя')
        elif kind == 'integer':
            low = _integer(field.get('min', -2147483647))
            high = _integer(field.get('max', 2147483647))
            if low == -2147483648 or low > high or not re.fullmatch('[+-]?[0-9]+', default) or not low <= int(default) <= high:
                raise ValueError('Неверные границы или начальное число')
        elif kind == 'choice':
            choices = field.get('choices')
            if not isinstance(choices, list) or not 1 <= len(choices) <= 64:
                raise ValueError('Неверный список значений')
            seen = set()
            for choice in choices:
                if not isinstance(choice, dict):
                    raise ValueError('Неверный вариант')
                value = _text(choice.get('value'), 2048, True)
                _text(choice.get('label'), 160)
                if value in seen:
                    raise ValueError('Повтор значения')
                seen.add(value)
            if default not in seen:
                raise ValueError('Начального значения нет в списке')
        elif kind != 'text':
            raise ValueError('Неизвестный тип поля')
    return data
