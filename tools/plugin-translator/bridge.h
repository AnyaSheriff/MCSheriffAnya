// Типы примитивного внутреннего моста; внешних возможностей не добавляет.
#include <stdint.h>
void *mcsa_call(int32_t op, void *data, int32_t length);
int32_t mcsa_length(void);
