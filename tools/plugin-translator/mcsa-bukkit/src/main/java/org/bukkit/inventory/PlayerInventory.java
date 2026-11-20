package org.bukkit.inventory;
public interface PlayerInventory extends Inventory {
 ItemStack getItemInMainHand();void setItemInMainHand(ItemStack stack);
 ItemStack getItemInOffHand();void setItemInOffHand(ItemStack stack);
 ItemStack[] getArmorContents();void setArmorContents(ItemStack[] armor);ItemStack getHelmet();
 default void setBoots(ItemStack stack){setItem(36,stack);}
 default void setLeggings(ItemStack stack){setItem(37,stack);}
 default void setChestplate(ItemStack stack){setItem(38,stack);}
 default void setHelmet(ItemStack stack){setItem(39,stack);}
}
