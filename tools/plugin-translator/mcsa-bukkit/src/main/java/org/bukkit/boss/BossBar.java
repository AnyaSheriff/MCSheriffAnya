package org.bukkit.boss;
import org.bukkit.entity.Player;
public interface BossBar {
 String getTitle();void setTitle(String title);
 BarColor getColor();void setColor(BarColor color);
 BarStyle getStyle();void setStyle(BarStyle style);
 double getProgress();void setProgress(double progress);
 void addPlayer(Player player);void removePlayer(Player player);void removeAll();java.util.List<Player> getPlayers();
 boolean isVisible();void setVisible(boolean visible);
}
