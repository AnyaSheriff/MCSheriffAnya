package org.bukkit.scoreboard;
public interface Team {
 String getName();void addEntry(String entry);boolean removeEntry(String entry);boolean hasEntry(String entry);java.util.Set<String> getEntries();
 void prefix(net.kyori.adventure.text.Component prefix);net.kyori.adventure.text.Component prefix();void suffix(net.kyori.adventure.text.Component suffix);net.kyori.adventure.text.Component suffix();
 void color(net.kyori.adventure.text.format.NamedTextColor color);net.kyori.adventure.text.format.NamedTextColor color();void unregister();Scoreboard getScoreboard();
}
