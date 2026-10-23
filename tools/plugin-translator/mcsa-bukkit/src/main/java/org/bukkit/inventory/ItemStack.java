package org.bukkit.inventory;
import org.bukkit.Material; public class ItemStack {private Material type;private int amount;public ItemStack(Material m){this(m,1);}public ItemStack(Material m,int n){type=m;amount=n;}public Material getType(){return type;}public int getAmount(){return amount;}public void setAmount(int n){amount=n;}public void setType(Material m){type=m;}}
