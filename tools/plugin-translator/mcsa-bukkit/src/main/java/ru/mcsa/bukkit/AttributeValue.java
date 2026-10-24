package ru.mcsa.bukkit;
public final class AttributeValue implements org.bukkit.attribute.Attribute {
 private final org.bukkit.NamespacedKey key;public AttributeValue(String name){key=org.bukkit.NamespacedKey.minecraft(name);}public org.bukkit.NamespacedKey getKey(){return key;}
}
