package me.clip.placeholderapi.expansion;
// Только тип отсутствующей soft-dependency. Интеграция не регистрируется ядром.
public abstract class PlaceholderExpansion {
 protected PlaceholderExpansion(){throw new IllegalStateException("PlaceholderAPI отсутствует; проверьте PluginManager.isPluginEnabled перед созданием расширения");}
 public abstract String getIdentifier();public abstract String getAuthor();public abstract String getVersion();public boolean persist(){return false;}public String onRequest(org.bukkit.OfflinePlayer player,String params){return null;}
 public final boolean register(){throw new IllegalStateException("PlaceholderAPI отсутствует");}public final boolean unregister(){throw new IllegalStateException("PlaceholderAPI отсутствует");}
}
