package org.bukkit.scoreboard;
public interface Scoreboard {Team getTeam(String name);java.util.Set<Team> getTeams();Team registerNewTeam(String name);Team getEntryTeam(String entry);}
