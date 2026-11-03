package org.bukkit.entity;
public interface AbstractHorse extends Animals,org.bukkit.inventory.InventoryHolder {boolean isTamed();void setTamed(boolean value);void setOwner(AnimalTamer owner);org.bukkit.inventory.AbstractHorseInventory getInventory();}
