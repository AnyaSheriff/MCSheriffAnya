package ru.mcsa.bukkit;
import org.bukkit.persistence.*;
public final class DataType<T> implements PersistentDataType<T,T> {
 private final Class<T> type;public DataType(Class<T> type){this.type=type;}public Class<T> getPrimitiveType(){return type;}public Class<T> getComplexType(){return type;}public T toPrimitive(T value,PersistentDataAdapterContext context){return type.cast(value);}public T fromPrimitive(T value,PersistentDataAdapterContext context){return type.cast(value);}
}
