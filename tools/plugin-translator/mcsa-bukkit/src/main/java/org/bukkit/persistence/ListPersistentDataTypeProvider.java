package org.bukkit.persistence;
public final class ListPersistentDataTypeProvider {
 public ListPersistentDataType<String,String> strings(){return new ru.mcsa.bukkit.ListDataType<>(PersistentDataType.STRING);}public ListPersistentDataType<Double,Double> doubles(){return new ru.mcsa.bukkit.ListDataType<>(PersistentDataType.DOUBLE);}
}
