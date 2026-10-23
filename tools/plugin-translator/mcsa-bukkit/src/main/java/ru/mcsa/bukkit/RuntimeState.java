package ru.mcsa.bukkit;
import org.bukkit.event.*;
public final class RuntimeState {
 public interface Dispatcher {void dispatch(Listener listener, Event event,int priority);}
 public static String defaultConfig="";
 public static Dispatcher dispatcher;public static org.bukkit.plugin.java.JavaPlugin plugin;public static final Scheduler scheduler=new Scheduler();
 private static final java.util.List<Listener> listeners=new java.util.ArrayList<>();
 public static final org.bukkit.plugin.PluginManager manager=(l,p)->{if(!listeners.contains(l))listeners.add(l);};
 public static void fire(Event e){for(int priority=0;priority<6;priority++)for(Listener l:new java.util.ArrayList<>(listeners))dispatcher.dispatch(l,e,priority);}
}
