package ru.mcsa.bukkit;
import org.bukkit.*;
public final class SimpleRegistry<T extends Keyed> implements Registry<T> {
 private final java.util.Map<NamespacedKey,T> entries=new java.util.LinkedHashMap<>();public SimpleRegistry(T[] values){for(T value:values)entries.put(value.getKey(),value);}public T get(NamespacedKey key){return entries.get(key);}public java.util.Iterator<T> iterator(){return java.util.Collections.unmodifiableCollection(entries.values()).iterator();}
}
