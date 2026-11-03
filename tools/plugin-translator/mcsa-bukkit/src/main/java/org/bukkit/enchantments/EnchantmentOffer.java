package org.bukkit.enchantments;
public class EnchantmentOffer {
 private final Enchantment enchantment;private final int level;private int cost;
 public EnchantmentOffer(Enchantment enchantment,int level,int cost){this.enchantment=java.util.Objects.requireNonNull(enchantment);this.level=level;setCost(cost);}public Enchantment getEnchantment(){return enchantment;}public int getEnchantmentLevel(){return level;}public int getCost(){return cost;}public void setCost(int value){if(value<=0||value>32767)throw new IllegalArgumentException("Стоимость зачарования: 1..32767");cost=value;}
}
