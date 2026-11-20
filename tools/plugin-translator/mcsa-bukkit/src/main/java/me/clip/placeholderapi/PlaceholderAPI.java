package me.clip.placeholderapi;

import org.bukkit.OfflinePlayer;
import org.bukkit.entity.Player;

// Связывание только для объявленной необязательной зависимости в ветке без PlaceholderAPI.
public final class PlaceholderAPI {
    private PlaceholderAPI() {}

    public static String setPlaceholders(OfflinePlayer player, String text) {
        throw new IllegalStateException("PlaceholderAPI отсутствует; проверьте PluginManager.isPluginEnabled перед вызовом");
    }

    public static String setPlaceholders(Player player, String text) {
        return setPlaceholders((OfflinePlayer)player, text);
    }
}
