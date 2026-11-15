"""Лёгкие проверки описания; не обращаются к ключу и не запускают подпись."""
import copy
import json
from pathlib import Path
import unittest
from settings_schema import validate_settings


class SettingsSchemaTests(unittest.TestCase):
    def setUp(self):
        self.schema = json.loads(Path(__file__).with_name('settings-schema.json').read_text())

    def test_real_four_fields(self):
        self.assertEqual(4, len(validate_settings(self.schema)['fields']))
        self.assertEqual({'enable-nether', 'enable-end', 'experimental-parallel-mobs', 'mob-tick-threads'}, {f['key'] for f in self.schema['fields']})

    def test_reject_malformed_values(self):
        for key, value in [('default', 'false\nother=true'), ('file', '../server'), ('key', 'debug'), ('key', 'bad_key'), ('type', 'code'), ('default', True), ('group', 'Обновления')]:
            with self.subTest(key=key, value=value):
                schema = copy.deepcopy(self.schema)
                schema['fields'][0][key] = value
                with self.assertRaises(ValueError):
                    validate_settings(schema)

    def test_integer_bounds(self):
        for key, value in [('min', -2147483648), ('max', 0.5), ('max', True), ('default', '9'), ('default', '0_0'), ('min', 9)]:
            with self.subTest(key=key, value=value):
                schema = copy.deepcopy(self.schema)
                schema['fields'][3][key] = value
                with self.assertRaises(ValueError):
                    validate_settings(schema)
        self.schema['fields'][3].pop('min')
        self.schema['fields'][3].pop('max')
        validate_settings(self.schema)

    def test_duplicates_count_and_size(self):
        self.schema['fields'].append(copy.deepcopy(self.schema['fields'][0]))
        with self.assertRaises(ValueError): validate_settings(self.schema)
        self.schema['fields'] *= 26
        with self.assertRaises(ValueError): validate_settings(self.schema)
        self.schema['fields'] = []
        self.schema['unused'] = 'x' * 35000
        with self.assertRaises(ValueError): validate_settings(self.schema)

    def test_choices(self):
        field = self.schema['fields'][0]
        field.update(type='choice', default='a', choices=[{'value':'a', 'label':'А'}])
        validate_settings(self.schema)
        field['choices'].append({'value':'a', 'label':'Повтор'})
        with self.assertRaises(ValueError): validate_settings(self.schema)


if __name__ == '__main__': unittest.main()
