package org.bukkit.inventory;
import org.bukkit.Material;import org.bukkit.enchantments.Enchantment;import org.bukkit.inventory.meta.ItemMeta;import ru.mcsa.bukkit.ItemMetadata;
public class ItemStack implements Cloneable {
 private java.util.function.Consumer<ItemStack> observer;
 public ItemStack observe(java.util.function.Consumer<ItemStack> observer){this.observer=observer;return this;}
 private void changed(){if(observer!=null)observer.accept(this);}
 private Material type;private int amount;private ItemMetadata meta;
 public ItemStack(Material material){this(material,1);}public ItemStack(Material material,int amount){type=java.util.Objects.requireNonNull(material);this.amount=amount;meta=ItemMetadata.create(material);}public Material getType(){return type;}public int getAmount(){return amount;}public void setAmount(int amount){this.amount=amount;changed();}public void setType(Material material){type=java.util.Objects.requireNonNull(material);meta=meta.copyFor(material);changed();}
 public ItemStack clone(){ItemStack copy=new ItemStack(type,amount);copy.meta=meta.clone();return copy;}public boolean isEmpty(){return type.isAir()||amount<=0;}public int getMaxStackSize(){return type.getMaxStackSize();}
 public ItemMeta getItemMeta(){return type.isAir()?null:meta.clone();}public boolean setItemMeta(ItemMeta value){if(value==null){meta=ItemMetadata.create(type);changed();return true;}if(!(value instanceof ItemMetadata))throw new IllegalArgumentException("Чужой ItemMeta");meta=((ItemMetadata)value).copyFor(type);changed();return true;}
 public boolean editMeta(java.util.function.Consumer<? super ItemMeta> consumer){ItemMeta copy=getItemMeta();if(copy==null)return false;consumer.accept(copy);return setItemMeta(copy);}
 public io.papermc.paper.persistence.PersistentDataContainerView getPersistentDataContainer(){return meta.getPersistentDataContainer().clone();}
 public java.util.Map<Enchantment,Integer> getEnchantments(){return meta.getEnchants();}public void addUnsafeEnchantment(Enchantment enchant,int level){meta.addEnchant(enchant,level,true);changed();}public int getEnchantmentLevel(Enchantment enchant){return getEnchantments().getOrDefault(enchant,0);}public boolean containsEnchantment(Enchantment enchant){return getEnchantments().containsKey(enchant);}
 public boolean isSimilar(ItemStack other){return other!=null&&type==other.type&&meta.equals(other.meta);}@Override public boolean equals(Object other){return other instanceof ItemStack&&amount==((ItemStack)other).amount&&isSimilar((ItemStack)other);}@Override public int hashCode(){return java.util.Objects.hash(type,amount,meta);}
 public String metadata(){return meta.serialize();}public static ItemStack fromHost(Material type,int count,String metadata){ItemStack item=new ItemStack(type,count);item.meta=ItemMetadata.parse(type,metadata);return item;}
}
