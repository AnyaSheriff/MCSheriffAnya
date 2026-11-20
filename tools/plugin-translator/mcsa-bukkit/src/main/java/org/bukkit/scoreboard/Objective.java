package org.bukkit.scoreboard;

public interface Objective {
    String getName();
    Scoreboard getScoreboard();
    Score getScore(String entry);
    void setDisplayName(String name);
    void setDisplaySlot(DisplaySlot slot);
    DisplaySlot getDisplaySlot();
    void unregister();
}
