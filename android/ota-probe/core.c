/* Только маленькая испытательная библиотека; это НЕ игровое ядро. */
#include <stdio.h>
#ifndef PROBE_VALUE
#define PROBE_VALUE 37
#endif
__attribute__((visibility("default"))) int mcsa_core_api_v1(void) { return 1; }
__attribute__((visibility("default"))) int mcsa_core_main_v1(void) {
    printf("MCSA_CORE_PROBE_%d\n", PROBE_VALUE);
    return PROBE_VALUE;
}
