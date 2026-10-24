package ru.mcsa.bukkit;
public final class SimpleSound implements org.bukkit.Sound {
 private final String name;private final org.bukkit.NamespacedKey key;private final int ordinal;
 public SimpleSound(String name,String key,int ordinal){this.name=name;this.key=org.bukkit.NamespacedKey.minecraft(key);this.ordinal=ordinal;}
 public String name(){return name;}public int ordinal(){return ordinal;}public org.bukkit.NamespacedKey getKey(){return key;}
 @Override public String toString(){return name;}
}
