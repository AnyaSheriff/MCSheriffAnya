// Регрессия: вывод C/WASI должен сохранять число при повторном разборе.
#include <assert.h>
#include <stdint.h>
#include "decimal-runtime.h"
int main(void) {
    const char *canonical[] = {"7.0", "0.5", "-1.0", "0.0", "-0.0", "10.5", "0.1", "1.0E20", "1.0E-7", "4.9E-324", "1.7976931348623157E308", "NaN", "Infinity"};
    char text[64];unsigned count = 0;
    for (unsigned i = 0; i < sizeof canonical / sizeof *canonical; i++) {
        mcsa_decimal_format(strtod(canonical[i], NULL), 0, text);
        assert(!strcmp(canonical[i], text));count++;
    }
    double parsed;
    for (unsigned i = 0; i < sizeof canonical / sizeof *canonical; i++) {
        assert(mcsa_decimal_parse(canonical[i], (int)strlen(canonical[i]), 0, &parsed));
        if (isnan(parsed)) assert(!strcmp(canonical[i], "NaN"));
        else {mcsa_decimal_format(parsed, 0, text);assert(!strcmp(text, canonical[i]));}count++;
    }
    const char *bad[] = {"", " ", ".", "e1", "1e", "1e+", "inf", "nan", "NaN(x)", "Infinityd", "0x1", "0x.p2", "0x1p", "1 2", "1ff", "--1"};
    for (unsigned i = 0; i < sizeof bad / sizeof *bad; i++) {assert(!mcsa_decimal_parse(bad[i], (int)strlen(bad[i]), 0, &parsed));count++;}
    assert(mcsa_decimal_parse("  +0x1.8p2F  ", 13, 0, &parsed) && parsed == 6);count++;
    uint64_t seed = UINT64_C(0xc203ab76219f4b7d);
    for (int i = 0; i < 20000; i++) {
        seed = seed * UINT64_C(6364136223846793005) + 1;
        union { uint64_t bits; double value; } before = {.bits = seed}, after;
        if (isnan(before.value)) continue;
        mcsa_decimal_format(before.value, 0, text);after.value = strtod(text, NULL);
        assert(before.bits == after.bits);count++;
    }
    for (int i = 0; i < 10000; i++) {
        seed = seed * UINT64_C(6364136223846793005) + 1;
        union { uint32_t bits; float value; } before = {.bits = (uint32_t)(seed >> 32)}, after;
        if (isnan(before.value)) continue;
        mcsa_decimal_format(before.value, 1, text);after.value = strtof(text, NULL);
        assert(before.bits == after.bits);count++;
    }
    printf("PASS: %u decimal cases, including signed zero, exponents and subnormals\n", count);
}
