package org.bukkit.event;
public final class HandlerList {
 private HandlerList(){}
 // По Javadoc Bukkit: "Unregister a specific listener from all handler lists."
 // https://jd.papermc.io/paper/1.21.11/org/bukkit/event/HandlerList.html
 public static void unregisterAll(Listener listener){ru.mcsa.bukkit.RuntimeState.unregister(listener);}
}
