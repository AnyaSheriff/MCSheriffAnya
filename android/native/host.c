/* Стабильный отдельный процесс: только публичный Android/NDK dlopen API.
 * В приложении путь обновления выбирается лишь после проверки подписи.
 * Библиотека остаётся отображённой до выхода процесса: Rust TLS не выгружаем.
 */
#include <dlfcn.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
    (void)argc;
    char bundled[PATH_MAX];
    const char *library = getenv("MCSA_CORE_LIBRARY");
    if (!library || !*library) {
        const char *slash = strrchr(argv[0], '/');
        if (!slash || snprintf(bundled, sizeof(bundled), "%.*s/libmcsa_core.so", (int)(slash - argv[0]), argv[0]) >= (int)sizeof(bundled)) {
            fputs("MCSheriffAnya: не удалось найти встроенное ядро\n", stderr);
            return 126;
        }
        library = bundled;
    }
    void *handle = dlopen(library, RTLD_NOW | RTLD_LOCAL);
    if (!handle) {
        fprintf(stderr, "MCSheriffAnya: загрузка ядра не удалась: %s\n", dlerror());
        return 126;
    }
    int (*abi)(void) = (int (*)(void))dlsym(handle, "mcsa_core_api_v1");
    int (*run)(void) = (int (*)(void))dlsym(handle, "mcsa_core_main_v1");
    if (!abi || !run || abi() != 1) {
        fputs("MCSheriffAnya: неподдерживаемый интерфейс ядра\n", stderr);
        return 126;
    }
    return run();
}
