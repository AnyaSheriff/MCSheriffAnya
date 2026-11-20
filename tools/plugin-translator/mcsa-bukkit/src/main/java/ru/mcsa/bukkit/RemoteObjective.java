package ru.mcsa.bukkit;

import io.papermc.paper.scoreboard.numbers.FixedFormat;
import io.papermc.paper.scoreboard.numbers.NumberFormat;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;
import org.bukkit.scoreboard.DisplaySlot;
import org.bukkit.scoreboard.Objective;
import org.bukkit.scoreboard.Score;
import org.bukkit.scoreboard.Scoreboard;

final class RemoteObjective implements Objective {
    private final MainScoreboard board;
    final String name;
    private final Map<String, Value> scores = new LinkedHashMap<>();
    private String title;
    DisplaySlot slot;
    private boolean registered = true;

    RemoteObjective(MainScoreboard board, String name, String title) {
        this.board = board;
        this.name = name;
        this.title = title;
    }

    private void check() {
        if (!registered) throw new IllegalStateException("Цель таблицы очков удалена");
    }

    public String getName() { check(); return name; }
    public Scoreboard getScoreboard() { return registered ? board : null; }
    public DisplaySlot getDisplaySlot() { check(); return slot; }
    public void setDisplaySlot(DisplaySlot slot) { check(); board.setSlot(this, slot); }

    public void setDisplayName(String value) {
        check();
        String next = Text.json(Text.legacy(value, '§'));
        for (String viewer : board.viewers()) MainScoreboard.requireApplied(Host.yes(170, viewer, name, next));
        title = next;
    }

    public Score getScore(String entry) {
        check();
        if (entry == null || entry.indexOf('\0') >= 0 || Strings.utf8(entry).length > 128) throw new IllegalArgumentException("Недопустимая запись счёта");
        return new EntryScore(entry);
    }

    public void unregister() {
        check();
        board.remove(this);
        registered = false;
        scores.clear();
    }

    Set<String> entries() { return scores.keySet(); }
    void send(String viewer) { MainScoreboard.requireApplied(Host.yes(170, viewer, name, title)); }
    void remove(String viewer) { MainScoreboard.requireApplied(Host.yes(171, viewer, name)); }

    void sync(String viewer) {
        send(viewer);
        for (Map.Entry<String, Value> score : scores.entrySet()) sendScore(viewer, score.getKey(), score.getValue());
    }

    void reset(String entry) {
        if (!scores.containsKey(entry)) return;
        for (String viewer : board.viewers()) MainScoreboard.requireApplied(Host.yes(174, viewer, name, entry));
        scores.remove(entry);
    }

    private void sendScore(String viewer, String entry, Value value) {
        String format = value.format == null ? "" : Text.json(value.format.component());
        MainScoreboard.requireApplied(Host.yes(173, viewer, name, entry, Integer.toString(value.points), format));
    }

    private void update(String entry, int value, FixedFormat format) {
        Value next = new Value(value, format);
        for (String viewer : board.viewers()) sendScore(viewer, entry, next);
        scores.put(entry, next);
    }

    private static final class Value {
        final int points;
        final FixedFormat format;
        Value(int points, FixedFormat format) { this.points = points; this.format = format; }
    }

    private final class EntryScore implements Score {
        private final String entry;
        EntryScore(String entry) { this.entry = entry; }
        public String getEntry() { return entry; }
        public Objective getObjective() { return RemoteObjective.this; }
        public boolean isScoreSet() { check(); return scores.containsKey(entry); }
        public int getScore() { check(); Value current = scores.get(entry); return current == null ? 0 : current.points; }
        public void setScore(int value) { check(); update(entry, value, numberFormat()); }
        public void resetScore() { check(); reset(entry); }
        public FixedFormat numberFormat() { check(); Value current = scores.get(entry); return current == null ? null : current.format; }
        public void numberFormat(NumberFormat format) {
            check();
            if (format != null && !(format instanceof FixedFormat)) throw new IllegalArgumentException("Поддерживается фиксированный формат счёта");
            update(entry, getScore(), (FixedFormat)format);
        }
    }
}
