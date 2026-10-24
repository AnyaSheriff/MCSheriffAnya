package org.bukkit.persistence;
import org.bukkit.NamespacedKey;
public interface PersistentDataContainer extends io.papermc.paper.persistence.PersistentDataContainerView {<P,C>void set(NamespacedKey key,PersistentDataType<P,C> type,C value);void remove(NamespacedKey key);PersistentDataAdapterContext getAdapterContext();void copyTo(PersistentDataContainer other,boolean replace);}
