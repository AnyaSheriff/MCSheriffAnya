package net.kyori.adventure.key;
public interface Key extends Comparable<Key> {
 String namespace(); String value(); default String asString(){return namespace()+":"+value();}
 default int compareTo(Key other){int n=value().compareTo(other.value());return n==0?namespace().compareTo(other.namespace()):n;}
 static Key key(String value){org.bukkit.NamespacedKey key=org.bukkit.NamespacedKey.fromString(value);if(key==null)throw new IllegalArgumentException("Неверный ключ");return key;}
}
