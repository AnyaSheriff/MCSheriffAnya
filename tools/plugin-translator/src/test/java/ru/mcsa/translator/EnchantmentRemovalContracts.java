package ru.mcsa.translator;
import java.util.*;
import org.bukkit.Material;
import org.bukkit.enchantments.Enchantment;
import org.bukkit.inventory.ItemStack;
import org.bukkit.inventory.meta.*;
import net.kyori.adventure.text.Component;
public final class EnchantmentRemovalContracts {
 private static int checks;
 private static void check(boolean value){checks++;if(!value)throw new AssertionError("check "+checks);}
 private static void rejectsNull(Runnable action){boolean rejected=false;try{action.run();}catch(NullPointerException expected){rejected=true;}check(rejected);}
 public static void main(String[] args){
  ItemStack item=new ItemStack(Material.DIAMOND_SWORD);int[] writes={0};item.observe(changed->writes[0]++);
  check(item.removeEnchantment(Enchantment.SHARPNESS)==0&&writes[0]==0);
  rejectsNull(()->item.removeEnchantment(null));rejectsNull(()->item.getItemMeta().removeEnchant(null));check(writes[0]==0);
  item.addUnsafeEnchantment(Enchantment.SHARPNESS,7);item.addUnsafeEnchantment(Enchantment.UNBREAKING,2);writes[0]=0;
  ItemMeta before=item.getItemMeta();before.displayName(Component.text("Keep"));before.lore(Arrays.asList(Component.text("Lore")));((Damageable)before).setDamage(9);before.getPersistentDataContainer().set(org.bukkit.NamespacedKey.minecraft("probe"),org.bukkit.persistence.PersistentDataType.STRING,"keep");item.setItemMeta(before);writes[0]=0;
  ItemMeta detached=item.getItemMeta();check(detached.removeEnchant(Enchantment.SHARPNESS));check(!detached.removeEnchant(Enchantment.SHARPNESS));
  check(item.getEnchantmentLevel(Enchantment.SHARPNESS)==7&&writes[0]==0);
  check(detached.getEnchantLevel(Enchantment.UNBREAKING)==2&&detached.hasDisplayName()&&((Damageable)detached).getDamage()==9);
  ItemMeta cloned=detached.clone();check(cloned.removeEnchant(Enchantment.UNBREAKING));check(!cloned.hasEnchants()&&detached.hasEnchants());
  check(item.removeEnchantment(Enchantment.SHARPNESS)==7&&writes[0]==1);check(item.removeEnchantment(Enchantment.SHARPNESS)==0&&writes[0]==1);
  check(item.getEnchantmentLevel(Enchantment.UNBREAKING)==2&&before.getEnchantLevel(Enchantment.SHARPNESS)==7);
  String withOne=item.metadata();ItemStack roundtrip=ItemStack.fromHost(item.getType(),item.getAmount(),withOne);check(roundtrip.getEnchantmentLevel(Enchantment.SHARPNESS)==0&&roundtrip.getEnchantmentLevel(Enchantment.UNBREAKING)==2);
  check(item.removeEnchantment(Enchantment.UNBREAKING)==2&&writes[0]==2);check(!item.getItemMeta().hasEnchants());
  check(!ru.mcsa.bukkit.Yaml.parse(item.metadata()).containsKey("enchants"));
  ItemMeta remaining=item.getItemMeta();check(remaining.hasDisplayName()&&remaining.lore().size()==1&&((Damageable)remaining).getDamage()==9);check("keep".equals(remaining.getPersistentDataContainer().get(org.bukkit.NamespacedKey.minecraft("probe"),org.bukkit.persistence.PersistentDataType.STRING)));
  check(!ItemStack.fromHost(item.getType(),1,item.metadata()).getItemMeta().hasEnchants());
  ItemStack copy=roundtrip.clone();check(copy.removeEnchantment(Enchantment.UNBREAKING)==2);check(roundtrip.getEnchantmentLevel(Enchantment.UNBREAKING)==2&&writes[0]==2);
  check(item.setItemMeta(null));check(!item.getItemMeta().hasEnchants()&&item.removeEnchantment(Enchantment.SHARPNESS)==0);
  ItemStack book=ItemStack.fromHost(Material.ENCHANTED_BOOK,1,"{\"stored\":{\"minecraft:unbreaking\":2},\"enchants\":{\"minecraft:sharpness\":3}}");
  check(book.removeEnchantment(Enchantment.SHARPNESS)==3);check(((EnchantmentStorageMeta)book.getItemMeta()).getStoredEnchants().get(Enchantment.UNBREAKING)==2);check(!book.getItemMeta().removeEnchant(Enchantment.UNBREAKING));check(((EnchantmentStorageMeta)book.getItemMeta()).getStoredEnchants().get(Enchantment.UNBREAKING)==2);
  ItemStack opaque=ItemStack.fromHost(Material.DIAMOND_SWORD,1,"{\"_native\":11,\"_opaque\":11,\"enchants\":{\"minecraft:sharpness\":3}}");check(opaque.removeEnchantment(Enchantment.SHARPNESS)==3);Map<?,?> fields=ru.mcsa.bukkit.Yaml.parse(opaque.metadata());check(fields.containsKey("_native")&&fields.containsKey("_opaque")&&!fields.containsKey("enchants"));
  System.out.println("PASS: "+checks+" enchantment removal contracts");
 }
}
