package ru.mcsa.translator;
import java.util.*;
import org.bukkit.Material;
import org.bukkit.enchantments.Enchantment;
import org.bukkit.inventory.ItemStack;
import org.bukkit.inventory.meta.*;
import net.kyori.adventure.text.Component;
public final class MetadataGuardContracts {
 private static int checks;
 private static void check(boolean value){checks++;if(!value)throw new AssertionError("check "+checks);}
 private static void rejectsNull(Runnable action){boolean rejected=false;try{action.run();}catch(NullPointerException expected){rejected=true;}check(rejected);}
 public static void main(String[] args){
  ItemStack item=new ItemStack(Material.DIAMOND_SWORD);ItemMeta meta=item.getItemMeta();
  check(!meta.hasDisplayName());check(!meta.hasEnchants());check(!meta.hasEnchant(Enchantment.SHARPNESS));check(meta.getEnchantLevel(Enchantment.SHARPNESS)==0);
  rejectsNull(()->meta.hasEnchant(null));rejectsNull(()->meta.getEnchantLevel(null));
  meta.displayName(Component.empty());check(meta.hasDisplayName());
  check(!item.getItemMeta().hasDisplayName());
  ItemMeta clone=meta.clone();check(clone.hasDisplayName());clone.displayName(null);check(!clone.hasDisplayName()&&meta.hasDisplayName());
  check(item.setItemMeta(meta));check(item.getItemMeta().hasDisplayName());
  ItemStack nameRoundtrip=ItemStack.fromHost(item.getType(),1,item.metadata());check(nameRoundtrip.getItemMeta().hasDisplayName());
  meta.displayName(null);meta.lore(Collections.emptyList());check(!meta.hasDisplayName()&&!meta.hasEnchants());
  check(!meta.addEnchant(Enchantment.SHARPNESS,0,false));check(!meta.hasEnchants()&&!meta.hasEnchant(Enchantment.SHARPNESS)&&meta.getEnchantLevel(Enchantment.SHARPNESS)==0);
  check(meta.addEnchant(Enchantment.SHARPNESS,3,false));check(meta.hasEnchants()&&meta.hasEnchant(Enchantment.SHARPNESS));check(meta.getEnchantLevel(Enchantment.SHARPNESS)==3);
  check(!meta.hasEnchant(Enchantment.UNBREAKING)&&meta.getEnchantLevel(Enchantment.UNBREAKING)==0);
  check(meta.addEnchant(Enchantment.UNBREAKING,2,false));check(meta.getEnchantLevel(Enchantment.SHARPNESS)==3&&meta.getEnchantLevel(Enchantment.UNBREAKING)==2);
  check(meta.addEnchant(Enchantment.SHARPNESS,7,true));check(meta.getEnchantLevel(Enchantment.SHARPNESS)==7);
  ItemMeta enchantedCopy=meta.clone();check(enchantedCopy.hasEnchants()&&enchantedCopy.hasEnchant(Enchantment.SHARPNESS)&&enchantedCopy.getEnchantLevel(Enchantment.SHARPNESS)==7);
  check(enchantedCopy.addEnchant(Enchantment.SHARPNESS,1,false));check(meta.getEnchantLevel(Enchantment.SHARPNESS)==7&&enchantedCopy.getEnchantLevel(Enchantment.SHARPNESS)==1);
  check(item.setItemMeta(meta));String before=item.metadata();
  ItemMeta read=item.getItemMeta();check(!read.hasDisplayName()&&read.hasEnchants()&&read.hasEnchant(Enchantment.SHARPNESS)&&read.getEnchantLevel(Enchantment.SHARPNESS)==7);
  check(item.metadata().equals(before));
  ItemMeta roundtrip=ItemStack.fromHost(item.getType(),1,before).getItemMeta();check(!roundtrip.hasDisplayName()&&roundtrip.hasEnchants()&&roundtrip.getEnchantLevel(Enchantment.SHARPNESS)==7&&roundtrip.getEnchantLevel(Enchantment.UNBREAKING)==2);
  check(item.setItemMeta(null));ItemMeta reset=item.getItemMeta();check(!reset.hasDisplayName()&&!reset.hasEnchants()&&!reset.hasEnchant(Enchantment.SHARPNESS)&&reset.getEnchantLevel(Enchantment.SHARPNESS)==0);
  check(meta.hasEnchants()&&meta.getEnchantLevel(Enchantment.SHARPNESS)==7);
  ItemMeta resetRoundtrip=ItemStack.fromHost(item.getType(),1,item.metadata()).getItemMeta();check(!resetRoundtrip.hasEnchants()&&!resetRoundtrip.hasDisplayName());
  ItemMeta stored=ItemStack.fromHost(Material.ENCHANTED_BOOK,1,"{\"stored\":{\"minecraft:unbreaking\":2}}").getItemMeta();
  check(((EnchantmentStorageMeta)stored).getStoredEnchants().get(Enchantment.UNBREAKING)==2);
  check(!stored.hasEnchants()&&!stored.hasEnchant(Enchantment.UNBREAKING)&&stored.getEnchantLevel(Enchantment.UNBREAKING)==0);
  System.out.println("PASS: "+checks+" metadata guard contracts");
 }
}
