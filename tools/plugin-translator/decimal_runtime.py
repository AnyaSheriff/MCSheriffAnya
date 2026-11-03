# Платформенный вывод double/float для C/WASI; входной JAR здесь не исполняется.
from pathlib import Path
import re
import shutil


def port_parsers(folder):
    exception_header = folder / 'classes/java/lang/NumberFormatException.h'
    if not exception_header.exists():
        return
    constructor = re.search(r'extern void (meth_jl_NumberFormatException__init_\w*)\(void\*\);', exception_header.read_text())
    if not constructor:
        raise ValueError('Неизвестный ABI TeaVM NumberFormatException')
    for name, primitive in [('Double', 'double'), ('Float', 'float')]:
        source = folder / ('classes/java/lang/' + name + '.c')
        if not source.exists():
            continue
        text = source.read_text()
        match = re.search(r'(?:double|float) meth_jl_' + name + r'_parse(?:Double|Float)\(void\* teavm_local_1\) \{', text)
        if not match:
            continue
        end, depth = match.end(), 1
        while depth:
            depth += (text[end] == '{') - (text[end] == '}')
            end += 1
        call_site = re.search(r'TEAVM_WITH_CALL_SITE_ID\((\d+)', text[match.end():end])[1]
        body = '''
    TEAVM_ALLOC_STACK(INT32_C(2));
    TEAVM_CALL_SITE(CALLSITE);
    TEAVM_GC_ROOT_RELEASE(INT32_C(1));
    TEAVM_GC_ROOT(INT32_C(0), teavm_local_1);
    if (!(((TeaVM_Class *)&jl_CLASS_Cls)->flags & INT32_C(1))) initclass_jl_CLASS();
    teavm_local_1 = teavm_nullCheck(teavm_local_1);
    int32_t length = TEAVM_METHOD(teavm_local_1, jl_String_VT, virt_length)(teavm_local_1);
    char *decimal = teavm_stringToC(teavm_local_1);
    double result;
    int valid = mcsa_decimal_parse(decimal, length, SINGLE, &result);
    free(decimal);
    if (!valid) {
        void *exception = meth_otr_Allocator_allocate(&jl_NumberFormatException_Cls);
        TEAVM_GC_ROOT(INT32_C(1), exception);
        CONSTRUCTOR(exception);
        meth_otr_ExceptionHandling_throwException(exception);
        TEAVM_UNREACHABLE
    }
    TEAVM_RELEASE_STACK;
    return (PRIMITIVE)result;
'''.replace('CALLSITE', call_site).replace('CLASS', name).replace('SINGLE', '1' if primitive == 'float' else '0').replace('CONSTRUCTOR', constructor[1]).replace('PRIMITIVE', primitive)
        source.write_text('#include "../../../mcsa_decimal.h"\n' + text[:match.end()] + body + '}' + text[end:])


def port_decimals(folder):
    shutil.copyfile(Path(__file__).with_name('decimal-runtime.h'), folder / 'mcsa_decimal.h')
    port_parsers(folder)
    source = folder / 'classes/java/lang/AbstractStringBuilder.c'
    if not source.exists():
        return
    text = source.read_text()
    methods = []
    pattern = r'void\* (meth_jl_AbstractStringBuilder_insert\w*)\(void\* teavm_this_, int32_t teavm_local_1, (void\*|double|float) teavm_local_2\) \{'
    for match in re.finditer(pattern, text):
        end, depth = match.end(), 1
        while depth:
            depth += (text[end] == '{') - (text[end] == '}')
            end += 1
        methods.append((match, end, text[match.end():end - 1]))
    numeric = [method for method in methods if method[0][2] in ('double', 'float')]
    if not numeric:
        return
    strings = [method for method in methods if method[0][2] == 'void*' and 'jl_String_VT' in method[2]]
    if len(strings) != 1:
        raise ValueError('Неизвестный ABI TeaVM StringBuilder: безопасный вывод чисел невозможен')
    insert_string = strings[0][0][1]
    for match, end, original in reversed(numeric):
        call_site = re.search(r'TEAVM_WITH_CALL_SITE_ID\((\d+)', original)[1]
        body = '''
    TEAVM_ALLOC_STACK(INT32_C(2));
    TEAVM_CALL_SITE(CALLSITE);
    TEAVM_GC_ROOT_RELEASE(INT32_C(1));
    TEAVM_GC_ROOT(INT32_C(0), teavm_this_);
    char decimal[64];
    mcsa_decimal_format((double)teavm_local_2, SINGLE, decimal);
    void *number = teavm_cToString(decimal);
    TEAVM_GC_ROOT(INT32_C(1), number);
    void *result = INSERT(teavm_this_, teavm_local_1, number);
    TEAVM_RELEASE_STACK;
    return result;
'''.replace('CALLSITE', call_site).replace('SINGLE', '1' if match[2] == 'float' else '0').replace('INSERT', insert_string)
        text = text[:match.end()] + body + '}' + text[end:]
    source.write_text('#include "../../../mcsa_decimal.h"\n' + text)
    shutil.copyfile(Path(__file__).with_name('decimal-runtime.h'), folder / 'mcsa_decimal.h')
