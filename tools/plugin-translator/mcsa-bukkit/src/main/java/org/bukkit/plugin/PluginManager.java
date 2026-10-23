package org.bukkit.plugin;
public interface PluginManager {
 void registerEvents(org.bukkit.event.Listener listener,Plugin plugin);
 default boolean isPluginEnabled(String name){Plugin plugin=getPlugin(name);return plugin!=null&&plugin.isEnabled();}
 default Plugin getPlugin(String name){Plugin plugin=ru.mcsa.bukkit.RuntimeState.plugin;return plugin!=null&&plugin.getName().equalsIgnoreCase(name)?plugin:null;}
}
