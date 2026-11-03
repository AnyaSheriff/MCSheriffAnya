package org.bukkit.scheduler;
public interface BukkitTask { int getTaskId(); org.bukkit.plugin.Plugin getOwner(); boolean isSync(); boolean isCancelled(); void cancel(); }
