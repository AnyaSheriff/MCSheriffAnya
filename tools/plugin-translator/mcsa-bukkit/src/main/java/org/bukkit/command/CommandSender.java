package org.bukkit.command;
public interface CommandSender extends net.kyori.adventure.audience.Audience { String getName(); void sendMessage(String message); default void sendMessage(net.kyori.adventure.text.Component message){sendMessage(((ru.mcsa.bukkit.Text)message).plain());} boolean isOp(); boolean hasPermission(String permission); }
