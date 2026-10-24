// Десятичный вывод WASI: кратчайшая запись с обратимым разбором, без таблиц TeaVM.
#ifndef MCSA_DECIMAL_RUNTIME_H
#define MCSA_DECIMAL_RUNTIME_H
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static int mcsa_decimal_digit(char value, int hex) {
    return (value >= '0' && value <= '9') || (hex && ((value >= 'a' && value <= 'f') || (value >= 'A' && value <= 'F')));
}
// Ограниченный ASCII-разбор грамматики Java, включая hex-float и суффиксы типов.
static int mcsa_decimal_parse(const char *text, int length, int single, double *result) {
    if (length < 0 || length > 4096 || strlen(text) != (size_t)length) return 0;
    for (int i = 0; i < length; i++) if ((unsigned char)text[i] > 127) return 0;
    while (length && (unsigned char)*text <= 32) { text++; length--; }
    while (length && (unsigned char)text[length - 1] <= 32) length--;
    char value[4097]; memcpy(value, text, length); value[length] = 0;
    char *p = value;
    int negative = *p == '-'; if (*p == '+' || *p == '-') p++;
    if (!strcmp(p, "NaN")) { *result = NAN; return 1; }
    if (!strcmp(p, "Infinity")) { *result = negative ? -INFINITY : INFINITY; return 1; }
    if (length && strchr("fFdD", value[length - 1])) value[--length] = 0;
    int hex = p[0] == '0' && (p[1] == 'x' || p[1] == 'X');
    if (hex) p += 2;
    int digits = 0;
    while (mcsa_decimal_digit(*p, hex)) { p++; digits++; }
    if (*p == '.') { p++; while (mcsa_decimal_digit(*p, hex)) { p++; digits++; } }
    if (!digits) return 0;
    if (hex ? (*p == 'p' || *p == 'P') : (*p == 'e' || *p == 'E')) {
        p++; if (*p == '+' || *p == '-') p++;
        int exponent_digits = 0;
        while (mcsa_decimal_digit(*p, 0)) { p++; exponent_digits++; }
        if (!exponent_digits) return 0;
    } else if (hex) return 0;
    if (*p) return 0;
    *result = single ? (double)strtof(value, NULL) : strtod(value, NULL);
    return 1;
}
static void mcsa_decimal_format(double value, int single, char out[64]) {
    if (isnan(value)) { strcpy(out, "NaN"); return; }
    if (isinf(value)) { strcpy(out, signbit(value) ? "-Infinity" : "Infinity"); return; }
    if (value == 0) { strcpy(out, signbit(value) ? "-0.0" : "0.0"); return; }
    double magnitude = fabs(value);
    char scientific[64], digits[24];
    int precision, limit = single ? 9 : 17;
    // В научной записи Java сохраняет хотя бы две значащие цифры.
    for (precision = 2; precision <= limit; precision++) {
        snprintf(scientific, sizeof scientific, "%.*e", precision - 1, magnitude);
        double parsed = strtod(scientific, NULL);
        if (single ? (float)parsed == (float)magnitude : parsed == magnitude) break;
    }
    char *exponent_at = strchr(scientific, 'e');
    if (!exponent_at) abort();
    int exponent = atoi(exponent_at + 1), count = 0;
    for (char *p = scientific; p < exponent_at; p++) if (*p != '.') digits[count++] = *p;
    while (count > 1 && digits[count - 1] == '0') count--;
    char *p = out;
    if (signbit(value)) *p++ = '-';
    if (exponent >= 7 || exponent < -3) {
        *p++ = digits[0]; *p++ = '.';
        if (count == 1) *p++ = '0';
        else for (int i = 1; i < count; i++) *p++ = digits[i];
        *p++ = 'E'; snprintf(p, 12, "%d", exponent);
    } else if (exponent < 0) {
        *p++ = '0'; *p++ = '.';
        for (int i = -1; i > exponent; i--) *p++ = '0';
        for (int i = 0; i < count; i++) *p++ = digits[i];
        *p = 0;
    } else {
        for (int i = 0; i <= exponent; i++) *p++ = i < count ? digits[i] : '0';
        *p++ = '.';
        if (count <= exponent + 1) *p++ = '0';
        else for (int i = exponent + 1; i < count; i++) *p++ = digits[i];
        *p = 0;
    }
}
#endif
