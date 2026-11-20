package org.bukkit.inventory.meta;
import net.kyori.adventure.text.Component;import org.bukkit.persistence.PersistentDataContainer;import org.bukkit.enchantments.Enchantment;import org.bukkit.inventory.ItemFlag;import java.util.*;
import net.kyori.adventure.text.serializer.legacy.LegacyComponentSerializer;
public interface ItemMeta extends Cloneable {boolean removeEnchant(Enchantment enchant);boolean hasDisplayName();boolean hasEnchants();boolean hasEnchant(Enchantment enchant);int getEnchantLevel(Enchantment enchant);void displayName(Component name);Component displayName();void lore(List<? extends Component> lore);List<Component> lore();PersistentDataContainer getPersistentDataContainer();boolean addEnchant(Enchantment enchant,int level,boolean ignoreRestrictions);Map<Enchantment,Integer> getEnchants();void addItemFlags(ItemFlag... flags);Set<ItemFlag> getItemFlags();ItemMeta clone();
 boolean isUnbreakable();void setUnbreakable(boolean unbreakable);
 default String getDisplayName(){
  Component name=displayName();
  return name==null?"":LegacyComponentSerializer.builder().hexColors().useUnusualXRepeatedCharacterHexFormat().build().serialize(name);
 }
 default void setDisplayName(String name){displayName(name==null?null:LegacyComponentSerializer.legacySection().deserialize(name));}
 default void setLore(List<String> lines){
  if(lines==null){lore(null);return;}
  LegacyComponentSerializer serializer=LegacyComponentSerializer.legacySection();
  List<Component> components=new ArrayList<>(lines.size());
  for(String line:lines)components.add(serializer.deserialize(line));
  lore(components);
 }
}
