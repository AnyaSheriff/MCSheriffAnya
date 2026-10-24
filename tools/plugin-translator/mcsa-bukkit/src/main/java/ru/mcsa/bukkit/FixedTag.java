package ru.mcsa.bukkit;
import org.bukkit.*;import java.util.*;
// Каталог закреплённой версии ядра; внешние наборы данных не загружаются.
public final class FixedTag<T extends Keyed> implements Tag<T> {
 private final NamespacedKey key;private final Set<T> values;
 public FixedTag(String name,Collection<T> values){key=NamespacedKey.minecraft(name);this.values=Collections.unmodifiableSet(new LinkedHashSet<>(values));}
 public NamespacedKey getKey(){return key;}public boolean isTagged(T value){return values.contains(value);}public Set<T> getValues(){return values;}
 public static Tag<Material> materials(String name,Collection<String> names){List<Material> result=new ArrayList<>();for(String value:names){Material material=Material.matchMaterial(value);if(material!=null)result.add(material);}return new FixedTag<>(name,result);}
 public static Tag<Material> materialSuffix(String name,String suffix){List<Material> result=new ArrayList<>();for(Material material:Material.values())if(material.name().endsWith(suffix))result.add(material);return new FixedTag<>(name,result);}
}
