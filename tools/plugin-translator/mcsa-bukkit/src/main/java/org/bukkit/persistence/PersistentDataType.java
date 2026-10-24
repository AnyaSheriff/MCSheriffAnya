package org.bukkit.persistence;
public interface PersistentDataType<P,C> {
 Class<P> getPrimitiveType();Class<C> getComplexType();P toPrimitive(C value,PersistentDataAdapterContext context);C fromPrimitive(P value,PersistentDataAdapterContext context);
 PersistentDataType<Byte,Byte> BYTE=new ru.mcsa.bukkit.DataType<>(Byte.class);
 PersistentDataType<Integer,Integer> INTEGER=new ru.mcsa.bukkit.DataType<>(Integer.class);
 PersistentDataType<Long,Long> LONG=new ru.mcsa.bukkit.DataType<>(Long.class);
 PersistentDataType<Double,Double> DOUBLE=new ru.mcsa.bukkit.DataType<>(Double.class);
 PersistentDataType<String,String> STRING=new ru.mcsa.bukkit.DataType<>(String.class);
 ListPersistentDataTypeProvider LIST=new ListPersistentDataTypeProvider();
}
