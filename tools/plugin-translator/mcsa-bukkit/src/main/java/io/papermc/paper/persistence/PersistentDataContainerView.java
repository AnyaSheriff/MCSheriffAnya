package io.papermc.paper.persistence;
import org.bukkit.*;import org.bukkit.persistence.*;
public interface PersistentDataContainerView {<P,C>C get(NamespacedKey key,PersistentDataType<P,C> type);<P,C>C getOrDefault(NamespacedKey key,PersistentDataType<P,C> type,C fallback);boolean has(NamespacedKey key);<P,C>boolean has(NamespacedKey key,PersistentDataType<P,C> type);boolean isEmpty();java.util.Set<NamespacedKey> getKeys();}
