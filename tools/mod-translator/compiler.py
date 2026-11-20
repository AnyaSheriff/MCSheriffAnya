"""Полный разбор собственного ModAPI source-subset; Java-код не исполняется."""
from dataclasses import dataclass
import json
import re


class Refusal(ValueError):
    def __init__(self, code, message, line=1, column=1):
        super().__init__(message)
        self.issue = dict(code=code, message=message, line=line, column=column)


@dataclass(frozen=True)
class Token:
    text: str
    line: int
    column: int
    string: bool = False


def lex(source):
    if len(source.encode('utf-8')) > 65536:
        raise Refusal('source-size', 'Исходник больше 64 КиБ')
    # Java раскрывает Unicode escapes до разбора комментариев. Здесь они запрещены
    # целиком, чтобы не получить другое поведение после обычного javac.
    if re.search(r'\\u+[0-9a-fA-F]{4}', source):
        raise Refusal('unicode-escape', 'Java Unicode escapes не входят в source-subset')
    tokens = []
    pos, line, column = 0, 1, 1
    while pos < len(source):
        rest = source[pos:]
        if rest.startswith('/*'):
            end = rest.find('*/', 2)
            if end < 0:
                raise Refusal('comment', 'Не закрыт комментарий', line, column)
            text = rest[:end + 2]
        elif rest.startswith('//'):
            text = rest.split('\n', 1)[0]
        elif rest[0] in ' \t\r\n\f':
            text = rest[0]
        elif rest[0] == '"':
            match = re.match(r'"(?:[^"\\\x00-\x1f]|\\["\\nrt])*"', rest)
            if not match:
                raise Refusal('string', 'Нужна строка с escapes \", \\\\, \\n, \\r, \\t', line, column)
            text = match.group()
            value = json.loads(text)
            if '\0' in value or len(value.encode('utf-8')) > 4096 or any(0xD800 <= ord(c) <= 0xDFFF for c in value):
                raise Refusal('string-size', 'Строка должна быть UTF-8 без NUL, до 4096 байт', line, column)
            tokens.append(Token(value, line, column, True))
        else:
            match = re.match(r'[A-Za-z_][A-Za-z_0-9]*|[0-9]+|->|[{}().,;+]', rest)
            if not match:
                raise Refusal('token', f'Неподдерживаемый символ: {rest[0]!r}', line, column)
            text = match.group()
            tokens.append(Token(text, line, column))
        pos += len(text)
        if '\n' in text:
            line += text.count('\n')
            column = len(text.rsplit('\n', 1)[-1]) + 1
        else:
            column += len(text)
    tokens.append(Token('<EOF>', line, column))
    return tokens


class Parser:
    def __init__(self, source):
        self.tokens = lex(source)
        self.index = 0

    @property
    def token(self):
        return self.tokens[self.index]

    def take(self, text=None):
        token = self.token
        if text is not None and (token.string or token.text != text):
            self.fail(f'Ожидалось «{text}», найдено «{token.text}»')
        self.index += 1
        return token

    def fail(self, message, code='unsupported-source'):
        raise Refusal(code, message, self.token.line, self.token.column)

    def name(self):
        if self.token.string or not re.fullmatch('[A-Za-z_][A-Za-z_0-9]*', self.token.text):
            self.fail('Ожидалось имя')
        if self.token.text in {'abstract', 'assert', 'boolean', 'break', 'byte', 'case', 'catch', 'char', 'class', 'const', 'continue', 'default', 'do', 'double', 'else', 'enum', 'extends', 'final', 'finally', 'float', 'for', 'goto', 'if', 'implements', 'import', 'instanceof', 'int', 'interface', 'long', 'native', 'new', 'package', 'private', 'protected', 'public', 'return', 'short', 'static', 'strictfp', 'super', 'switch', 'synchronized', 'this', 'throw', 'throws', 'transient', 'try', 'void', 'volatile', 'while', 'true', 'false', 'null', '_', 'var', 'yield', 'record', 'sealed', 'permits'}:
            self.fail('Ключевое слово Java не может быть именем')
        return self.take().text

    def parse(self):
        imports = set()
        while self.token.text == 'import':
            self.take('import')
            parts = [self.name()]
            while self.token.text == '.':
                self.take('.')
                parts.append(self.name())
            name = '.'.join(parts)
            if name not in {'ru.mcsa.modapi.Mod', 'ru.mcsa.modapi.Context'} or name in imports:
                self.fail(f'API «{name}» не поддерживается; нужны только Mod и Context собственного SDK', 'unsupported-api')
            imports.add(name)
            self.take(';')
        if imports != {'ru.mcsa.modapi.Mod', 'ru.mcsa.modapi.Context'}:
            self.fail('Нужны явные imports Mod и Context собственного SDK')
        self.take('public'); self.take('final'); self.take('class')
        class_name = self.name()
        if class_name in {'Mod', 'Context'}:
            self.fail('Имя класса конфликтует с импортом собственного SDK')
        self.take('implements'); self.take('Mod'); self.take('{')
        self.take('public'); self.take('void'); self.take('initialize'); self.take('(')
        self.take('Context'); self.context = self.name(); self.take(')'); self.take('{')
        callbacks = []
        commands = set()
        while self.token.text != '}':
            callback = self.registration()
            if callback['event'] == 'onCommand':
                name = callback['command']
                if name in commands:
                    self.fail(f'Команда «{name}» уже объявлена', 'duplicate-command')
                commands.add(name)
            callbacks.append(callback)
            if len(callbacks) > 64:
                self.fail('Не более 64 обработчиков', 'registration-limit')
        self.take('}'); self.take('}'); self.take('<EOF>')
        return dict(class_name=class_name, callbacks=callbacks)

    def registration(self):
        self.take(self.context); self.take('.')
        event = self.name()
        if event not in {'onStart', 'onStop', 'onJoin', 'onQuit', 'onChat', 'onCommand', 'every'}:
            self.fail(f'Регистрация «{event}» не поддерживается', 'unsupported-event')
        self.take('(')
        period = None
        extra = {}
        if event == 'every':
            token = self.take()
            if token.string or not re.fullmatch('[1-9][0-9]{0,6}', token.text) or not 1 <= int(token.text) <= 2400000:
                self.fail('Период every должен быть целым 1..2400000', 'period')
            period = int(token.text)
            self.take(',')
        if event == 'onCommand':
            if not self.token.string or not re.fullmatch('[a-z0-9_-]{1,32}', self.token.text):
                self.fail('Имя команды: строка из 1..32 символов a-z, 0-9, _ и -', 'command-name')
            extra['command'] = self.take().text
            self.take(',')
            if self.token.string or not re.fullmatch('[0-4]', self.token.text):
                self.fail('Уровень прав команды — целое число от 0 до 4', 'command-level')
            extra['level'] = int(self.take().text)
            self.take(',')
        player = None
        if event in {'onJoin', 'onQuit', 'onChat', 'onCommand'}:
            player = self.name()
            if player == self.context:
                self.fail('Имя параметра должно отличаться от Context')
        else:
            self.take('('); self.take(')')
        self.take('->'); self.take('{')
        actions = []
        replied = False
        while self.token.text != '}':
            if event == 'onChat' and self.token.text == player:
                self.take(player); self.take('.')
                action = self.name()
                if action != 'cancel':
                    self.fail(f'Изменение чата «{action}» не поддерживается; доступно только cancel()', 'unsupported-chat-action')
                self.take('('); self.take(')'); self.take(';')
                actions.append(dict(operation='cancel', expression=[]))
                if len(actions) > 64:
                    self.fail('Не более 64 действий в обработчике', 'action-limit')
                continue
            self.take(self.context); self.take('.')
            operation = self.name()
            if operation not in {'broadcast', 'log', 'message', 'reply'}:
                self.fail(f'Действие «{operation}» не поддерживается', 'unsupported-action')
            self.take('(')
            if operation == 'message':
                if event == 'onChat':
                    self.fail('Для onChat личное сообщение пока не поддерживается; используйте log/broadcast', 'unsupported-chat-action')
                if event not in {'onJoin', 'onQuit'}:
                    self.fail('message доступно только в обработчике входа или выхода игрока')
                self.take(player)
                self.take(',')
            if operation == 'reply':
                if event != 'onCommand':
                    self.fail('reply доступно только внутри onCommand', 'unsupported-command-action')
                if replied:
                    self.fail('Команда может вернуть только один reply', 'duplicate-reply')
                self.take(player)
                self.take(',')
                replied = True
            expression = []
            while True:
                if self.token.string:
                    expression.append(('literal', self.take().text))
                elif player and self.token.text == player:
                    self.take(player); self.take('.')
                    field = self.name()
                    allowed = {
                        'onChat': {'message', 'playerName', 'playerUuid'},
                        'onCommand': {'name', 'senderName', 'argument'},
                    }.get(event, {'name', 'uuid'})
                    if field not in allowed:
                        if event == 'onCommand':
                            self.fail(f'Поле команды «{field}» не поддерживается', 'unsupported-command-field')
                        self.fail(f'Поле игрока «{field}» не поддерживается', 'unsupported-player-field')
                    self.take('(')
                    if field == 'argument':
                        if self.token.string or not re.fullmatch('0|[1-9]|[12][0-9]|3[01]', self.token.text):
                            self.fail('Номер аргумента — целое число от 0 до 31', 'command-argument')
                        expression.append(('argument', int(self.take().text)))
                    else:
                        kind = {'onChat': 'chat', 'onCommand': 'command'}.get(event, 'player')
                        expression.append((kind, field))
                    self.take(')')
                else:
                    self.fail('Нужно строковое выражение с поддержанными полями текущего события')
                if len(expression) > 32:
                    self.fail('Не более 32 частей строки', 'expression-limit')
                if self.token.text != '+':
                    break
                self.take('+')
            self.take(')'); self.take(';')
            actions.append(dict(operation=operation, expression=expression))
            if len(actions) > 64:
                self.fail('Не более 64 действий в обработчике', 'action-limit')
        self.take('}'); self.take(')'); self.take(';')
        return dict(event=event, period=period, actions=actions, **extra)


def parse(source):
    return Parser(source).parse()


def c_string(text):
    # Фиксированные octal escapes исключают неоднозначность C hex escapes.
    raw = text.encode('utf-8')
    return '"' + ''.join(f'\\{byte:03o}' for byte in raw) + '"', len(raw)


def generate(program):
    lines = ['#include "plugin.h"', '#include <stdlib.h>', '#include <string.h>',
             'static bool active;', 'static char buffer[131073];', 'static size_t used;',
             'static void append(const void *p, size_t n) { if(n > sizeof(buffer)-1-used) abort(); if(n) memcpy(buffer+used,p,n); used+=n; }']
    callbacks = program['callbacks']
    commands = [(i, c) for i, c in enumerate(callbacks) if c['event'] == 'onCommand']
    for i, callback in enumerate(callbacks):
        player = callback['event'] in {'onJoin', 'onQuit'}
        chat = callback['event'] == 'onChat'
        command = callback['event'] == 'onCommand'
        parameters = 'mcsa_api_types_player_t *who' if player else 'void'
        if chat:
            parameters = 'mcsa_api_types_player_t *who, plugin_string_t *message, bool *cancelled'
        if command:
            parameters = 'mcsa_api_types_sender_t *caller, plugin_string_t *name, plugin_list_string_t *args, mcsa_api_types_command_result_t *ret'
        lines.append(f'static void callback_{i}({parameters}) {{')
        if player or chat:
            lines.append('(void)who;')
        if chat:
            lines.append('(void)message; (void)cancelled;')
        if command:
            lines.append('(void)caller; (void)name; (void)args; (void)ret;')
        for action in callback['actions']:
            if action['operation'] == 'cancel':
                lines.append('*cancelled=true;')
                continue
            lines.append('used=0;')
            for kind, value in action['expression']:
                if kind == 'literal':
                    literal, size = c_string(value)
                    lines.append(f'append({literal},{size});')
                elif kind == 'chat':
                    field = {'message': 'message', 'playerName': 'who->name', 'playerUuid': 'who->uuid'}[value]
                    access = 'message->' if field == 'message' else field + '.'
                    lines.append(f'append({access}ptr,{access}len);')
                elif kind == 'command':
                    access = 'name->' if value == 'name' else 'caller->name.'
                    lines.append(f'append({access}ptr,{access}len);')
                elif kind == 'argument':
                    lines.append(f'if(args->len>{value}) append(args->ptr[{value}].ptr,args->ptr[{value}].len);')
                else:
                    lines.append(f'append(who->{value}.ptr,who->{value}.len);')
            # Блок сохраняет одинаковое имя переменной для нескольких действий.
            if action['operation'] == 'reply':
                lines.append('ret->message.is_some=true;')
                lines.append('if(used) plugin_string_dup_n(&ret->message.val,buffer,used);')
            else:
                call = dict(broadcast='mcsa_api_server_broadcast(&text)', log='mcsa_api_log_write(MCSA_API_LOG_LEVEL_INFO,&text)', message='(void)mcsa_api_player_send_message(&who->uuid,&text)')[action['operation']]
                lines.append('{ plugin_string_t text={(uint8_t*)buffer,used}; ' + call + '; }')
        lines.append('}')
    every = [i for i, c in enumerate(callbacks) if c['event'] == 'every']
    if every:
        lines.append(f'static uint32_t handles[{len(every)}];')
    lines.append('void exports_mcsa_api_lifecycle_enable(void) { if(active) return; active=true;')
    for _, command in commands:
        literal, size = c_string(command['command'])
        lines.append('{ plugin_string_t err; '
                     f'mcsa_api_command_command_spec_t spec={{.name={{(uint8_t*){literal},{size}}},.level={command["level"]}}}; '
                     'if(!mcsa_api_command_register(&spec,&err)) { mcsa_api_log_write(MCSA_API_LOG_LEVEL_ERROR,&err); plugin_string_free(&err); abort(); } }')
    for slot, i in enumerate(every):
        period = callbacks[i]['period']
        lines.append(f'handles[{slot}]=mcsa_api_scheduler_run_repeating({period},{period},{i+1});')
    for i, c in enumerate(callbacks):
        if c['event'] == 'onStart':
            lines.append(f'callback_{i}();')
    lines.append('}')
    lines.append('void exports_mcsa_api_lifecycle_disable(void) { if(!active) return; active=false;')
    for slot in range(len(every)):
        lines.append(f'(void)mcsa_api_scheduler_cancel(handles[{slot}]);')
    for i, c in enumerate(callbacks):
        if c['event'] == 'onStop':
            lines.append(f'callback_{i}();')
    lines.append('}')
    for event, wit in [('onJoin','join'), ('onQuit','quit')]:
        lines.append(f'void exports_mcsa_api_events_on_player_{wit}(mcsa_api_types_player_t *who) {{ (void)who; if(!active) return;')
        for i, c in enumerate(callbacks):
            if c['event'] == event:
                lines.append(f'callback_{i}(who);')
        lines.append('}')
    lines.append('void exports_mcsa_api_events_on_task(uint32_t task) { if(!active) return; switch(task) {')
    for i in every:
        lines.append(f'case {i+1}: callback_{i}(); break;')
    lines.append('default: break; }}')
    lines.append('void exports_mcsa_api_events_on_chat(mcsa_api_types_player_t *who, plugin_string_t *message, mcsa_api_types_chat_result_t *ret) { (void)who; (void)message; bool cancelled=false; if(active) {')
    for i, callback in enumerate(callbacks):
        if callback['event'] == 'onChat':
            lines.append(f'callback_{i}(who,message,&cancelled);')
    lines.append('} ret->tag=cancelled ? MCSA_API_TYPES_CHAT_RESULT_CANCEL : MCSA_API_TYPES_CHAT_RESULT_PASS; }')
    lines.append('void exports_mcsa_api_events_on_command(mcsa_api_types_sender_t *caller,plugin_string_t *name,plugin_list_string_t *args,mcsa_api_types_command_result_t *ret) { memset(ret,0,sizeof(*ret));')
    if commands:
        for i, command in commands:
            literal, size = c_string(command['command'])
            lines.append(f'if(active && name->len=={size} && !memcmp(name->ptr,{literal},{size})) {{')
            lines.append(f'ret->success=true; callback_{i}(caller,name,args,ret);')
            lines.append('}')
        # Параметры принадлежат гостю, ответ освобождает сгенерированный post-return.
        lines.append('mcsa_api_types_sender_free(caller); plugin_string_free(name); plugin_list_string_free(args);')
    else:
        lines.append('(void)caller;(void)name;(void)args;')
    lines.append('}')
    lines.extend([
        'mcsa_api_types_verdict_t exports_mcsa_api_events_on_block_break(mcsa_api_types_player_t *who,mcsa_api_types_block_pos_t *pos,plugin_string_t *block) { (void)who;(void)pos;(void)block;return MCSA_API_TYPES_VERDICT_ALLOW; }',
        'mcsa_api_types_verdict_t exports_mcsa_api_events_on_block_place(mcsa_api_types_player_t *who,mcsa_api_types_block_pos_t *pos,plugin_string_t *block) { (void)who;(void)pos;(void)block;return MCSA_API_TYPES_VERDICT_ALLOW; }',
        'mcsa_api_types_verdict_t exports_mcsa_api_events_on_player_command(mcsa_api_types_player_t *who,plugin_string_t *line) { (void)who;(void)line;return MCSA_API_TYPES_VERDICT_ALLOW; }'])
    return '\n'.join(lines) + '\n'
