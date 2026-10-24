package org.bukkit;
public interface World {
 org.bukkit.entity.Item dropItem(Location location,org.bukkit.inventory.ItemStack item);org.bukkit.entity.Item dropItemNaturally(Location location,org.bukkit.inventory.ItemStack item);
 void spawnParticle(Particle particle,Location loc,int count,double dx,double dy,double dz);
 boolean isChunkLoaded(int x,int z);int getHighestBlockYAt(Location location);<T>T getGameRuleValue(GameRule<T> rule);void playSound(Location location,Sound sound,float volume,float pitch);
 enum Environment {NORMAL,NETHER,THE_END,CUSTOM}
 Environment getEnvironment();java.util.List<org.bukkit.entity.Player> getPlayers();int getMinHeight();int getMaxHeight();
 default Material getType(int x,int y,int z){return getBlockAt(x,y,z).getType();}
 String getName(); WorldBorder getWorldBorder(); org.bukkit.block.Block getBlockAt(int x,int y,int z); Location getSpawnLocation(); long getTime(); void setTime(long time); boolean hasStorm(); void setStorm(boolean storm); boolean isThundering(); void setThundering(boolean thunder); }
