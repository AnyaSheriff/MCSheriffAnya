package ru.mcsa.modapi;

/**
 * Исходный текст и личность отправителя на время одного callback.
 * cancel необратим внутри мода; остальные его обработчики всё равно вызываются.
 * Изменение текста, снятие отмены и хранение объекта после callback не поддержаны.
 */
public interface ChatEvent {
    String message();
    String playerName();
    String playerUuid();
    void cancel();
}
