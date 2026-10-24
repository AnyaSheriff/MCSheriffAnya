package ru.mcsa.bukkit.logging;
public final class Logger {
 public static Logger getLogger(String name){return new Logger();}
 public void info(String text){ru.mcsa.bukkit.Host.call(23,text);}public void warning(String text){ru.mcsa.bukkit.Host.call(23,"WARNING: "+text);}public void severe(String text){ru.mcsa.bukkit.Host.call(23,"SEVERE: "+text);}
}
