package ru.mcsa.modapi;

/** Команда собственного мода. Данные доступны только во время обработчика. */
public interface CommandEvent {
    String name();
    String senderName();

    /** Первый аргумент имеет номер 0, отсутствующий аргумент возвращает пустую строку. */
    String argument(int index);
}
