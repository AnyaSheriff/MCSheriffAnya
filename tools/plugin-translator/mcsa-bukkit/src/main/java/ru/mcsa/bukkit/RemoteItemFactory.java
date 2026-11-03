package ru.mcsa.bukkit;
public final class RemoteItemFactory implements org.bukkit.inventory.ItemFactory {
 public org.bukkit.inventory.ItemStack enchantWithLevels(org.bukkit.inventory.ItemStack item,int levels,boolean treasure,java.util.Random random){java.util.Objects.requireNonNull(item);java.util.Objects.requireNonNull(random);String[] a=Host.call(113,Ascii.lower(item.getType().name()),""+item.getAmount(),item.metadata(),""+levels,""+treasure,""+random.nextLong());return org.bukkit.inventory.ItemStack.fromHost(org.bukkit.Material.matchMaterial(a[0]),Integer.parseInt(a[1]),a[2]);}
}
