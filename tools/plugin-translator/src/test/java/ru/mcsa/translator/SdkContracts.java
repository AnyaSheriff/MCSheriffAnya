package ru.mcsa.translator;
import org.bukkit.NamespacedKey;
import net.kyori.adventure.text.*;
import net.kyori.adventure.text.format.*;
import net.kyori.adventure.text.serializer.legacy.LegacyComponentSerializer;
import ru.mcsa.bukkit.Text;
public final class SdkContracts {
 private static void check(boolean value){if(!value)throw new AssertionError();}
 public static void main(String[] args){
  check(NamespacedKey.minecraft("a/b").toString().equals("minecraft:a/b"));
  check(NamespacedKey.fromString("STONE")==null);
  check(NamespacedKey.fromString("..:y")!=null);
  check(NamespacedKey.fromString("x:y:z")==null);
  check(NamespacedKey.fromString(":stone").equals(NamespacedKey.minecraft("stone")));
  check(NamespacedKey.minecraft("stone").hashCode()==NamespacedKey.fromString("stone").hashCode());
  TextComponent text=Component.text("A\n\"\\");
  TextComponent changed=text.append(Component.text("B")).decoration(TextDecoration.ITALIC,false);
  check(((Text)text).plain().equals("A\n\"\\"));
  check(((Text)changed).plain().equals("A\n\"\\B"));
  check(Text.json(text).equals("{\"text\":\"A\\u000a\\\"\\\\\"}"));
  check(Text.json(changed).contains("\"italic\":false"));
  TextComponent legacy=LegacyComponentSerializer.legacyAmpersand().deserialize("&aGreen&lBold&rWhite");
  check(((Text)legacy).plain().equals("GreenBoldWhite"));
  check(Text.json(legacy).contains("#55ff55"));
  check(Text.json(legacy).contains("\"bold\":true"));
  check(((Text)LegacyComponentSerializer.legacyAmpersand().deserialize("A&zB&")).plain().equals("A&zB&"));
  check(NamedTextColor.NAMES.value("white")==NamedTextColor.WHITE);
  check(Component.text("same").append(Component.text("child")).equals(Component.text("same").append(Component.text("child"))));
  org.bukkit.configuration.file.YamlConfiguration config=new org.bukkit.configuration.file.YamlConfiguration();
  config.loadFromString("root:\n  number: 4\n  list: [ONE:0, 'two', 3]\n  entries:\n    - when: NEAR\n      nested: {radius: 8, bonus: -10}\n    - when: OTHER\n      enabled: true\n  quoted: \"line\\nnext\"\n");
  check(config.getInt("root.number")==4);
  check(config.getStringList("root.list").equals(java.util.List.of("ONE:0","two","3")));
  check(config.getConfigurationSection("root").getKeys(false).contains("entries"));
  org.bukkit.configuration.file.YamlConfiguration copy=new org.bukkit.configuration.file.YamlConfiguration();copy.loadFromString(config.saveToString());
  check(copy.saveToString().equals(config.saveToString()));
  copy.set("root.number",9);check(copy.getInt("root.number")==9&&config.getInt("root.number")==4);
  for(String invalid:new String[]{"a: 1\na: 2", "a: [broken", "a: &ref value", "a: \"unclosed", "a: \t1", "a: [x}]"}) {boolean failed=false;try{copy.loadFromString(invalid);}catch(IllegalArgumentException expected){failed=true;}check(failed);}
  check(copy.getInt("root.number")==9);
  org.bukkit.inventory.ItemStack item=new org.bukkit.inventory.ItemStack(org.bukkit.Material.DIAMOND_SWORD);
  org.bukkit.inventory.meta.ItemMeta meta=item.getItemMeta();NamespacedKey marker=NamespacedKey.fromString("test:ability");
  meta.getPersistentDataContainer().set(marker,org.bukkit.persistence.PersistentDataType.STRING,"blink");
  meta.displayName(Component.text("Blade").color(NamedTextColor.RED));
  ((org.bukkit.inventory.meta.Damageable)meta).setDamage(3);meta.addEnchant(org.bukkit.enchantments.Enchantment.SHARPNESS,2,false);
  item.setItemMeta(meta);meta.getPersistentDataContainer().remove(marker);
  check(item.getPersistentDataContainer().get(marker,org.bukkit.persistence.PersistentDataType.STRING).equals("blink"));
  check(!item.getPersistentDataContainer().has(marker,org.bukkit.persistence.PersistentDataType.INTEGER));
  org.bukkit.inventory.ItemStack restored=org.bukkit.inventory.ItemStack.fromHost(item.getType(),item.getAmount(),item.metadata());
  check(item.equals(restored)&&item.hashCode()==restored.hashCode());
  check(((org.bukkit.inventory.meta.Damageable)restored.getItemMeta()).getDamage()==3);
  check(restored.getEnchantmentLevel(org.bukkit.enchantments.Enchantment.SHARPNESS)==2);
  restored.editMeta(m->m.getPersistentDataContainer().set(marker,org.bukkit.persistence.PersistentDataType.STRING,"other"));check(!restored.isSimilar(item));
  org.bukkit.persistence.PersistentDataContainer data=new ru.mcsa.bukkit.DataContainer();java.util.List<String> list=new java.util.ArrayList<>(java.util.List.of("one","two"));data.set(marker,org.bukkit.persistence.PersistentDataType.LIST.strings(),list);list.add("three");check(data.get(marker,org.bukkit.persistence.PersistentDataType.LIST.strings()).size()==2);
  java.util.List<String> retrieved=data.get(marker,org.bukkit.persistence.PersistentDataType.LIST.strings());retrieved.clear();check(data.get(marker,org.bukkit.persistence.PersistentDataType.LIST.strings()).size()==2);
  data.set(marker,org.bukkit.persistence.PersistentDataType.INTEGER,42);check(data.get(marker,org.bukkit.persistence.PersistentDataType.INTEGER)==42);
  check(org.bukkit.Material.DIAMOND_SWORD.getMaxDurability()==1561&&org.bukkit.Material.DIAMOND_SWORD.getMaxStackSize()==1);
  check(org.bukkit.Material.APPLE.isEdible()&&org.bukkit.Material.STONE.isSolid());
  check(org.bukkit.enchantments.Enchantment.SHARPNESS.conflictsWith(org.bukkit.enchantments.Enchantment.SMITE));
  System.out.println("PASS: 40 контрактов текста, YAML, PDC, копирования ItemMeta и свойств предметов");
 }
}
