package org.bukkit.scoreboard;

import io.papermc.paper.scoreboard.numbers.NumberFormat;

public interface Score {
    String getEntry();
    Objective getObjective();
    int getScore();
    boolean isScoreSet();
    void setScore(int value);
    void resetScore();
    NumberFormat numberFormat();
    void numberFormat(NumberFormat format);
}
