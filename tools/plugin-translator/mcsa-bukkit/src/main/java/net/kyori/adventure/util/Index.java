package net.kyori.adventure.util;
public final class Index<K,V> {
 private final java.util.Map<K,V> values;
 public Index(java.util.Map<K,V> values){this.values=new java.util.LinkedHashMap<>(values);}
 public V value(K key){return values.get(key);} public K key(V value){for(java.util.Map.Entry<K,V> e:values.entrySet())if(e.getValue().equals(value))return e.getKey();return null;}
}
