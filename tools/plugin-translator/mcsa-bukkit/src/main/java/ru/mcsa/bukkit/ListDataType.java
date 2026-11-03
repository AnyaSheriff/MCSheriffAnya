package ru.mcsa.bukkit;
import org.bukkit.persistence.*;import java.util.*;
public final class ListDataType<P,C> implements ListPersistentDataType<P,C> {
 private final PersistentDataType<P,C> element;public ListDataType(PersistentDataType<P,C> element){this.element=element;}public PersistentDataType<P,C> elementType(){return element;}
 @SuppressWarnings({"unchecked","rawtypes"}) public Class<List<P>> getPrimitiveType(){return (Class)List.class;}@SuppressWarnings({"unchecked","rawtypes"}) public Class<List<C>> getComplexType(){return (Class)List.class;}
 public List<P> toPrimitive(List<C> value,PersistentDataAdapterContext context){List<P> out=new ArrayList<>();for(C item:value)out.add(element.toPrimitive(item,context));return out;}
 public List<C> fromPrimitive(List<P> value,PersistentDataAdapterContext context){List<C> out=new ArrayList<>();for(P item:value)out.add(element.fromPrimitive(item,context));return out;}
}
