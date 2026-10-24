package net.kyori.adventure.text.serializer.legacy;
import net.kyori.adventure.text.*;import net.kyori.adventure.text.format.*;
public final class LegacyComponentSerializer {

 private final java.util.Map<String,TextComponent> cache=new java.util.LinkedHashMap<>();
 private final char marker;private LegacyComponentSerializer(char marker){this.marker=marker;}
 public static LegacyComponentSerializer legacyAmpersand(){return new LegacyComponentSerializer('&');}public static LegacyComponentSerializer legacySection(){return new LegacyComponentSerializer('§');}
 public TextComponent deserialize(String value){TextComponent found=cache.get(value);if(found!=null)return found;TextComponent result=ru.mcsa.bukkit.Text.legacy(value,marker);if(cache.size()>=256)cache.remove(cache.keySet().iterator().next());cache.put(value,result);return result;}
}
