package ru.sheriffanya.mcsa;

/** Один замок связывает запуск процесса и запись конфигов. */
final class HostState {
    static final Object LOCK = new Object();
    enum Phase { STOPPED, STARTING, RUNNING, STOPPING }
    static Phase phase = Phase.STOPPED;
    static String apiSession;
    static boolean dashboardVisible;
    static CoreStatsClient.Snapshot stats;
    static String operation;
    static String backupMessage="Копии хранятся в приложении. Экспортируй важные копии в память телефона.";
    static long backupRevision;
    static String message = "Сервер выключен";
    private static final StringBuilder log = new StringBuilder();
    static long revision;
    static void append(String text) {
        synchronized (LOCK) {
            log.append(text);
            if (log.length() > 65536) log.delete(0, log.length() - 65536);
            revision++;
        }
    }
    static String logs() { synchronized (LOCK) { return log.toString(); } }
    static boolean idle() { synchronized (LOCK) { return phase == Phase.STOPPED && operation == null; } }
}
