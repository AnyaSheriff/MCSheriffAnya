package ru.mcsa.modapi;

import java.util.function.Consumer;

/**
 * Регистрация идёт один раз при загрузке; обработчики одного события идут
 * в порядке регистрации. onStart вызывается после регистрации задач.
 * every начинает работу через period тактов; задачи отменяются до onStop.
 * Это только контракт для IDE/javac. Исполнение обеспечивает source-переводчик.
 */
public interface Context {
    void onStart(Runnable action);
    void onStop(Runnable action);
    void onJoin(Consumer<Player> action);
    void onQuit(Consumer<Player> action);
    void onChat(Consumer<ChatEvent> action);
    void onCommand(String name, int level, Consumer<CommandEvent> action);
    void every(int period, Runnable action);
    void broadcast(String text);
    void log(String text);
    void message(Player player, String text);
    void reply(CommandEvent command, String text);
}
