package org.bukkit.plugin;
public interface PluginManager {
 default void callEvent(org.bukkit.event.Event event){ru.mcsa.bukkit.RuntimeState.fire(event);}
 void registerEvents(org.bukkit.event.Listener listener,Plugin plugin);
 default boolean isPluginEnabled(String name){Plugin plugin=getPlugin(name);return plugin!=null&&plugin.isEnabled();}
 default Plugin getPlugin(String name){Plugin plugin=ru.mcsa.bukkit.RuntimeState.plugin;return plugin!=null&&plugin.getName().equalsIgnoreCase(name)?plugin:null;}
}
