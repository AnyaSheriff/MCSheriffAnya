package org.bukkit.scoreboard;
import java.util.Set;
import net.kyori.adventure.text.Component;

public interface Scoreboard {
    Team getTeam(String name);
    Set<Team> getTeams();
    Team registerNewTeam(String name);
    Team getEntryTeam(String entry);
    Objective getObjective(String name);
    Objective registerNewObjective(String name, String criteria, String displayName);
    Objective registerNewObjective(String name, String criteria, Component displayName);
    Set<String> getEntries();
    void resetScores(String entry);
}
