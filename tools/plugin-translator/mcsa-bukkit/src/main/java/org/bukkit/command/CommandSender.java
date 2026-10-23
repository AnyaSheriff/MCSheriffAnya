package org.bukkit.command;
public interface CommandSender { String getName(); void sendMessage(String message); boolean isOp(); boolean hasPermission(String permission); }
