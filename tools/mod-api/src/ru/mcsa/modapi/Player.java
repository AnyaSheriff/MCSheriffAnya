package ru.mcsa.modapi;

/** Неизменяемый снимок игрока в текущем событии. */
public interface Player {
    String name();
    String uuid();
}
