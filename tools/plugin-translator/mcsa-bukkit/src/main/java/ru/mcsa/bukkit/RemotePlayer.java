package ru.mcsa.bukkit;
import org.bukkit.*;import org.bukkit.entity.Player;import org.bukkit.inventory.*;
public final class RemotePlayer implements Player {
 private final String uuid,name;private final Location location;private final GameMode mode;
 public RemotePlayer(String[] a,int i){uuid=a[i];name=a[i+1];mode=GameMode.values()[Integer.parseInt(a[i+2])];location=new Location(new RemoteWorld(Integer.parseInt(a[i+3])),Double.parseDouble(a[i+4]),Double.parseDouble(a[i+5]),Double.parseDouble(a[i+6]),Float.parseFloat(a[i+7]),Float.parseFloat(a[i+8]));}
 @Override public boolean equals(Object other){return other instanceof RemotePlayer&&uuid.equals(((RemotePlayer)other).uuid);}@Override public int hashCode(){return uuid.hashCode();}
 public void setSilent(boolean value){Host.call(117,uuid,value?"1":"0");}public boolean isSilent(){return Host.yes(118,uuid);}
 public org.bukkit.entity.Entity getVehicle(){String id=Host.text(116,uuid);return id.isEmpty()?null:RemoteEntity.find(id);}
 public boolean isInsideVehicle(){return Host.yes(115,uuid);}
 public boolean getAllowFlight(){return Host.call(89,uuid)[0].equals("1");}
 public boolean isFlying(){return Host.call(89,uuid)[1].equals("1");}
 public net.kyori.adventure.util.TriState hasFlyingFallDamage(){return net.kyori.adventure.util.TriState.values()[Integer.parseInt(Host.call(89,uuid)[2])+1];}
 public void setAllowFlight(boolean value){Host.call(90,uuid,"0",value?"1":"0");}
 public void setFlying(boolean value){Host.call(90,uuid,"1",value?"1":"0");}
 public void setFlyingFallDamage(net.kyori.adventure.util.TriState value){java.util.Objects.requireNonNull(value);Host.call(90,uuid,"2",""+(value.ordinal()-1));}
 private String[] state(){return Host.call(65,uuid);}
 public int getFireTicks(){return Integer.parseInt(state()[0]);}public void setFireTicks(int ticks){Host.call(66,uuid,""+ticks);}
 public void damage(double amount){if(!Double.isFinite(amount)||amount<0)throw new IllegalArgumentException("Недопустимый урон");Host.call(67,uuid,""+amount);}
 public void giveExp(int amount){Host.call(68,uuid,""+amount);}
 public boolean isDead(){return state()[1].equals("1");}public boolean isSneaking(){return state()[2].equals("1");}public boolean isOnGround(){return state()[3].equals("1");}
 public boolean isInWater(){return state()[4].equals("1");}public boolean isInLava(){return state()[5].equals("1");}public boolean isInRain(){return state()[6].equals("1");}
 public double getEyeHeight(){return Double.parseDouble(state()[7]);}public Location getEyeLocation(){return getLocation().add(0,getEyeHeight(),0);}
 public boolean isValid(){return isOnline();}public boolean canSee(Player player){return player!=null&&player.isOnline();}
 public ItemStack getActiveItem(){String[] s=state();if(!s[8].equals("1"))return new ItemStack(Material.AIR);return s[9].equals("1")?getInventory().getItemInOffHand():getInventory().getItemInMainHand();}
 public java.util.Collection<org.bukkit.potion.PotionEffect> getActivePotionEffects(){String[] s=state();java.util.List<org.bukkit.potion.PotionEffect> result=new java.util.ArrayList<>();for(int i=10;i<s.length;i+=6){org.bukkit.potion.PotionEffectType type=org.bukkit.potion.PotionEffectType.getByName(s[i]);if(type!=null)result.add(new org.bukkit.potion.PotionEffect(type,Integer.parseInt(s[i+1]),Integer.parseInt(s[i+2]),s[i+3].equals("1"),s[i+4].equals("1"),s[i+5].equals("1")));}return result;}
 public org.bukkit.potion.PotionEffect getPotionEffect(org.bukkit.potion.PotionEffectType type){for(org.bukkit.potion.PotionEffect effect:getActivePotionEffects())if(effect.getType()==type)return effect;return null;}
 public org.bukkit.entity.EntityType getType(){return org.bukkit.entity.EntityType.PLAYER;}
 public String getName(){return name;}public java.util.UUID getUniqueId(){return java.util.UUID.fromString(uuid);}
 public Location snapshotLocation(){return location.clone();}
 public Location getLocation(){for(Player p:Bukkit.getOnlinePlayers())if(p.getUniqueId().toString().equals(uuid))return ((RemotePlayer)p).location.clone();return location.clone();}
 public World getWorld(){return getLocation().getWorld();} public GameMode getGameMode(){for(Player p:Bukkit.getOnlinePlayers())if(p.getUniqueId().toString().equals(uuid))return ((RemotePlayer)p).mode;return mode;}
 public void setGameMode(GameMode m){Host.call(11,uuid,""+m.ordinal());}public double getHealth(){return Double.parseDouble(Host.text(12,uuid));}
 public void sendMessage(String m){Host.call(2,uuid,m);}public boolean isOp(){return Integer.parseInt(Host.text(10,uuid))>0;}
 public void sendMessage(net.kyori.adventure.text.Component m){if(!Host.yes(27,uuid,Text.json(m)))throw new IllegalStateException("Компонент сообщения отклонён хозяином");}
 public void sendActionBar(net.kyori.adventure.text.Component m){if(!Host.yes(28,uuid,Text.json(m)))throw new IllegalStateException("Компонент action bar отклонён хозяином");}
 public org.bukkit.attribute.AttributeInstance getAttribute(org.bukkit.attribute.Attribute attribute){if(java.util.Arrays.asList("follow_range","flying_speed","spawn_reinforcements","tempt_range").contains(attribute.getKey().getKey()))return null;return new RemoteAttribute(uuid,attribute);}
 public void setHealth(double health){if(!Double.isFinite(health)||health<0)throw new IllegalArgumentException("Здоровье должно быть конечным и неотрицательным");Host.call(29,uuid,""+health);}
 public boolean addPotionEffect(org.bukkit.potion.PotionEffect effect){return Host.yes(30,uuid,effect.getType().getKey().getKey(),""+effect.getDuration(),""+effect.getAmplifier(),""+effect.isAmbient(),""+effect.hasParticles(),""+effect.hasIcon());}
 public void removePotionEffect(org.bukkit.potion.PotionEffectType type){Host.call(31,uuid,type.getKey().getKey());}
 public int getFoodLevel(){return Integer.parseInt(Host.call(62,uuid)[0]);}public int getLevel(){return Integer.parseInt(Host.call(62,uuid)[1]);}public float getExp(){return Float.parseFloat(Host.call(62,uuid)[2]);}
 public void setRespawnLocation(Location location){Location value=location==null?getLocation():location;Host.call(64,uuid,""+((RemoteWorld)value.getWorld()).dimension,""+value.getX(),""+value.getY(),""+value.getZ(),""+value.getYaw(),location==null?"0":"1");}
 public boolean hasPermission(String p){return Permissions.has(p,isOp());}
 public boolean teleport(Location l){return Host.yes(3,uuid,""+((RemoteWorld)l.getWorld()).dimension,""+l.getX(),""+l.getY(),""+l.getZ(),""+l.getYaw(),""+l.getPitch());}
 public void kickPlayer(String r){Host.call(20,uuid,r);}
 public InventoryView openInventory(Inventory inv){return ChestViews.open(this,inv);}public InventoryView getOpenInventory(){return ChestViews.current(this);}public void closeInventory(){ChestViews.close(this);}
 public EntityEquipment getEquipment(){return slot->{switch(slot){case HAND:return getInventory().getItemInMainHand();case OFF_HAND:return getInventory().getItemInOffHand();case FEET:return getInventory().getItem(36);case LEGS:return getInventory().getItem(37);case CHEST:return getInventory().getItem(38);case HEAD:return getInventory().getHelmet();default:return null;}};}
 public PlayerInventory getInventory(){return new PlayerItems(this);}
}
