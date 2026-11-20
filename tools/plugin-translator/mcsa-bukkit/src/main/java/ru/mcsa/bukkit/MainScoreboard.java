package ru.mcsa.bukkit;

import java.util.Collections;
import java.util.EnumMap;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.UUID;
import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.format.NamedTextColor;
import org.bukkit.Bukkit;
import org.bukkit.entity.Player;
import org.bukkit.scoreboard.*;

public final class MainScoreboard implements Scoreboard {
    public static final MainScoreboard INSTANCE = new MainScoreboard();
    public static final ScoreboardManager MANAGER = new ScoreboardManager() {
        public Scoreboard getMainScoreboard() { return INSTANCE; }
        public Scoreboard getNewScoreboard() { return new MainScoreboard(); }
    };
    private static final Map<UUID, MainScoreboard> assigned = new HashMap<>();
    private final Map<String, GuestTeam> teams = new LinkedHashMap<>();
    private final Map<String, RemoteObjective> objectives = new LinkedHashMap<>();
    private final Map<DisplaySlot, RemoteObjective> slots = new EnumMap<>(DisplaySlot.class);

    private MainScoreboard() {}

    public static MainScoreboard forPlayer(UUID uuid) {
        return assigned.getOrDefault(uuid, INSTANCE);
    }

    public static void leave(UUID uuid) {
        assigned.remove(uuid);
    }

    public static void assign(Player player, Scoreboard board) {
        if (!(board instanceof MainScoreboard)) throw new IllegalArgumentException("Неизвестная таблица очков");
        UUID uuid = player.getUniqueId();
        MainScoreboard previous = forPlayer(uuid);
        MainScoreboard next = (MainScoreboard)board;
        if (previous == next) return;
        if (!player.isOnline()) throw new IllegalStateException("Игрок отсутствует");
        String viewer = uuid.toString();
        try {
            previous.clearViewer(viewer);
            next.sync(player);
        } catch (IllegalStateException failure) {
            try {
                next.clearViewer(viewer);
                previous.sync(player);
            } catch (IllegalStateException rollback) {
                failure.addSuppressed(rollback);
            }
            throw failure;
        }
        if (next == INSTANCE) assigned.remove(uuid);
        else assigned.put(uuid, next);
    }

    public Team getTeam(String name) { return teams.get(Objects.requireNonNull(name)); }
    public Set<Team> getTeams() { return Collections.unmodifiableSet(new LinkedHashSet<Team>(teams.values())); }
    public Team getEntryTeam(String entry) {
        for (GuestTeam team : teams.values()) if (team.hasEntry(entry)) return team;
        return null;
    }

    public Team registerNewTeam(String name) {
        requireId(name);
        if (teams.containsKey(name)) throw new IllegalArgumentException("Имя команды занято");
        if (teams.size() >= 256) throw new IllegalStateException("Лимит команд: 256");
        GuestTeam team = new GuestTeam(name);
        team.send();
        teams.put(name, team);
        return team;
    }

    public Objective getObjective(String name) { return objectives.get(Objects.requireNonNull(name)); }

    public Objective registerNewObjective(String name, String criteria, String title) {
        return registerNewObjective(name, criteria, Text.legacy(title, '§'));
    }

    public Objective registerNewObjective(String name, String criteria, Component title) {
        requireId(name);
        if (!"dummy".equals(criteria)) throw new UnsupportedOperationException("Поддерживается критерий dummy");
        if (objectives.containsKey(name)) throw new IllegalArgumentException("Имя цели занято");
        if (objectives.size() >= 32) throw new IllegalStateException("Лимит целей: 32");
        RemoteObjective objective = new RemoteObjective(this, name, Text.json(title));
        for (String viewer : viewers()) objective.send(viewer);
        objectives.put(name, objective);
        return objective;
    }

    public Set<String> getEntries() {
        Set<String> entries = new LinkedHashSet<>();
        for (RemoteObjective objective : objectives.values()) entries.addAll(objective.entries());
        return Collections.unmodifiableSet(entries);
    }

    public void resetScores(String entry) {
        Objects.requireNonNull(entry);
        for (RemoteObjective objective : objectives.values()) objective.reset(entry);
    }

    void setSlot(RemoteObjective objective, DisplaySlot slot) {
        DisplaySlot old = objective.getDisplaySlot();
        if (old == slot) return;
        for (String viewer : viewers()) {
            if (old != null) Host.yes(172, viewer, old.protocolName(), "");
            if (slot != null) sendSlot(viewer, slot, objective);
        }
        if (old != null) slots.remove(old);
        if (slot != null) {
            RemoteObjective displaced = slots.put(slot, objective);
            if (displaced != null) displaced.slot = null;
        }
        objective.slot = slot;
    }

    void remove(RemoteObjective objective) {
        for (String viewer : viewers()) objective.remove(viewer);
        if (objective.slot != null) slots.remove(objective.slot);
        objectives.remove(objective.name);
    }

    private static void sendSlot(String viewer, DisplaySlot slot, RemoteObjective objective) {
        requireApplied(Host.yes(172, viewer, slot.protocolName(), objective.name));
    }

    public void sync(Player player) {
        String viewer = player.getUniqueId().toString();
        for (GuestTeam team : teams.values()) team.send(viewer);
        for (RemoteObjective objective : objectives.values()) objective.sync(viewer);
        for (Map.Entry<DisplaySlot, RemoteObjective> slot : slots.entrySet()) sendSlot(viewer, slot.getKey(), slot.getValue());
    }

    private void clearViewer(String viewer) {
        for (GuestTeam team : teams.values()) team.remove(viewer);
        for (RemoteObjective objective : objectives.values()) objective.remove(viewer);
    }

    Set<String> viewers() {
        Set<String> result = new LinkedHashSet<>();
        for (Player player : Bukkit.getOnlinePlayers()) {
            UUID uuid = player.getUniqueId();
            if (forPlayer(uuid) == this) result.add(uuid.toString());
        }
        return result;
    }

    static void requireApplied(boolean accepted) {
        if (!accepted) throw new IllegalStateException("Изменение таблицы очков отклонено ядром");
    }

    private static void requireId(String name) {
        if (name == null || name.isEmpty() || Strings.utf8(name).length > 128) {
            throw new IllegalArgumentException("Недопустимое имя таблицы очков");
        }
        for (int i = 0; i < name.length(); i++) {
            if (Character.isISOControl(name.charAt(i))) throw new IllegalArgumentException("Управляющий символ в имени");
        }
    }

    private final class GuestTeam implements Team {
        private final String name;
        private final Set<String> entries = new LinkedHashSet<>();
        private Component prefix = Component.empty(), suffix = Component.empty();
        private NamedTextColor color;
        private OptionStatus visibility = OptionStatus.ALWAYS;
        private boolean registered = true;

        GuestTeam(String name) { this.name = name; }
        private void check() { if (!registered) throw new IllegalStateException("Команда удалена"); }
        public String getName() { check(); return name; }
        public Scoreboard getScoreboard() { return registered ? MainScoreboard.this : null; }
        public boolean hasEntry(String entry) { check(); return entries.contains(Objects.requireNonNull(entry)); }
        public Set<String> getEntries() { check(); return Collections.unmodifiableSet(new LinkedHashSet<>(entries)); }
        public void addEntry(String entry) {
            check();
            if (entry == null || entry.indexOf('\0') >= 0 || Strings.utf8(entry).length > 128) throw new IllegalArgumentException("Недопустимая запись команды");
            if (entries.contains(entry)) return;
            if (entries.size() >= 1024) throw new IllegalStateException("Лимит записей команды: 1024");
            Team old = getEntryTeam(entry);
            if (old != null) old.removeEntry(entry);
            entries.add(entry);
            send();
        }
        public boolean removeEntry(String entry) {
            check();
            if (!entries.remove(Objects.requireNonNull(entry))) return false;
            send();
            return true;
        }
        public void prefix(Component value) { check(); prefix = Objects.requireNonNull(value); send(); }
        public Component prefix() { check(); return prefix; }
        public void suffix(Component value) { check(); suffix = Objects.requireNonNull(value); send(); }
        public Component suffix() { check(); return suffix; }
        public void color(NamedTextColor value) { check(); color = value; send(); }
        public NamedTextColor color() { check(); return color; }
        public OptionStatus getOption(Option option) { check(); Objects.requireNonNull(option); return visibility; }
        public void setOption(Option option, OptionStatus value) {
            check();
            Objects.requireNonNull(option);
            visibility = Objects.requireNonNull(value);
            send();
        }
        public void unregister() {
            check();
            for (String viewer : viewers()) remove(viewer);
            teams.remove(name);
            registered = false;
        }
        void remove(String viewer) { requireApplied(Host.yes(59, viewer, name)); }
        void send() { for (String viewer : viewers()) send(viewer); }
        void send(String viewer) {
            String visibleTo;
            switch (visibility) {
                case ALWAYS: visibleTo = "always"; break;
                case NEVER: visibleTo = "never"; break;
                case FOR_OTHER_TEAMS: visibleTo = "hideForOwnTeam"; break;
                case FOR_OWN_TEAM: visibleTo = "hideForOtherTeams"; break;
                default: throw new AssertionError(visibility);
            }
            java.util.List<String> args = new java.util.ArrayList<>();
            Collections.addAll(args, viewer, name, Text.json(prefix), Text.json(suffix), color == null ? "reset" : color.toString(), visibleTo);
            args.addAll(entries);
            requireApplied(Host.yes(58, args.toArray(new String[0])));
        }
    }
}
