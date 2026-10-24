package org.bukkit.event.enchantment;
public class PrepareItemEnchantEvent extends org.bukkit.event.Event implements org.bukkit.event.Cancellable {
 private final org.bukkit.entity.Player player;private final org.bukkit.inventory.ItemStack item;private final org.bukkit.enchantments.EnchantmentOffer[] offers;private boolean cancelled;
 public PrepareItemEnchantEvent(org.bukkit.entity.Player player,org.bukkit.inventory.ItemStack item,org.bukkit.enchantments.EnchantmentOffer[] offers){this.player=player;this.item=item;this.offers=offers;}
 public org.bukkit.entity.Player getEnchanter(){return player;}public org.bukkit.inventory.ItemStack getItem(){return item;}public org.bukkit.enchantments.EnchantmentOffer[] getOffers(){return offers;}public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
