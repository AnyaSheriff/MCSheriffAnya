package ru.mcsa.bukkit;
import org.bukkit.*;import org.bukkit.entity.*;import org.bukkit.util.Vector;import java.util.*;
public class RemoteEntity implements Entity {
 protected final String uuid;protected RemoteEntity(String uuid){this.uuid=uuid;}
 protected String[] snapshot(){return Host.call(94,uuid);}
 public UUID getUniqueId(){return UUID.fromString(uuid);}public EntityType getType(){return EntityType.valueOf(Ascii.upper(snapshot()[1]));}public boolean isValid(){return Host.yes(84,uuid);}
 public Location getLocation(){String[] s=snapshot();return new Location(new RemoteWorld(Integer.parseInt(s[2])),Double.parseDouble(s[3]),Double.parseDouble(s[4]),Double.parseDouble(s[5]),Float.parseFloat(s[6]),Float.parseFloat(s[7]));}public World getWorld(){return getLocation().getWorld();}
 public boolean teleport(Location at){return Host.yes(99,uuid,""+((RemoteWorld)at.getWorld()).dimension,""+at.getX(),""+at.getY(),""+at.getZ(),""+at.getYaw(),""+at.getPitch());}
 public static Entity find(String uuid){Player player=Bukkit.getPlayer(UUID.fromString(uuid));if(player!=null)return player;return from(Host.call(94,uuid),0);}
 public static Entity from(String[] s,int i){String uuid=s[i],kind=s[i+1];
  if(kind.equals("area_effect_cloud"))return new Cloud(uuid);if(kind.equals("firework_rocket"))return new Rocket(uuid);if(kind.equals("item_frame")||kind.equals("glow_item_frame"))return new Frame(uuid);
  if(kind.equals("player"))return Bukkit.getPlayer(UUID.fromString(uuid));if(kind.equals("item"))return new RemoteItem(uuid);
  if(kind.equals("arrow")||kind.equals("spectral_arrow")||kind.equals("trident"))return new Arrow(uuid);
  if(kind.equals("splash_potion")||kind.equals("lingering_potion"))return new Potion(uuid);
  if(kind.equals("fireball"))return new Fire(uuid);if(kind.equals("small_fireball"))return new SmallFire(uuid);
  if(kind.equals("wind_charge"))return new Wind(uuid);
  if(java.util.Arrays.asList("fireball","small_fireball","dragon_fireball","snowball","egg","ender_pearl","experience_bottle","wither_skull","llama_spit","breeze_wind_charge").contains(kind))return new Shot(uuid);
  if(java.util.Arrays.asList("horse","skeleton_horse","zombie_horse").contains(kind))return new Horse(uuid);if(kind.equals("donkey")||kind.equals("mule")||kind.equals("llama")||kind.equals("trader_llama"))return new Chested(uuid);if(kind.equals("camel"))return new Horse(uuid);
  if(s[i+11].equals("1")){if(kind.equals("vex"))return new VexEntity(uuid);if(kind.equals("armor_stand"))return new Stand(uuid);if(kind.equals("allay"))return new AllayEntity(uuid);if(s[i+12].equals("1"))return new Hostile(uuid);if(s[i+13].equals("1"))return new Animal(uuid);return new BasicMob(uuid);}
  return new RemoteEntity(uuid);
 }
 public static List<Entity> world(RemoteWorld world){String[] states=Host.call(95,""+world.dimension);List<Entity> result=new ArrayList<>();for(int i=0;i<states.length;i+=16){Entity entity=from(states,i);if(entity!=null)result.add(entity);}return result;}
 public static List<Entity> nearby(Entity who,double x,double y,double z){if(!Double.isFinite(x)||!Double.isFinite(y)||!Double.isFinite(z)||x<0||y<0||z<0)throw new IllegalArgumentException("Размеры поиска должны быть конечными и неотрицательными");Location at=who.getLocation();List<Entity> result=new ArrayList<>();String[] states=Host.call(95,""+((RemoteWorld)at.getWorld()).dimension);double width=who instanceof RemoteEntity?Double.parseDouble(((RemoteEntity)who).snapshot()[8]):0.6,height=who instanceof RemoteEntity?Double.parseDouble(((RemoteEntity)who).snapshot()[9]):1.8;
  for(int i=0;i<states.length;i+=16){if(states[i].equals(who.getUniqueId().toString()))continue;double bx=Double.parseDouble(states[i+3]),by=Double.parseDouble(states[i+4]),bz=Double.parseDouble(states[i+5]),bw=Double.parseDouble(states[i+8]),bh=Double.parseDouble(states[i+9]);if(Math.abs(bx-at.getX())<=x+(width+bw)/2&&Math.abs(bz-at.getZ())<=z+(width+bw)/2&&by+bh>=at.getY()-y&&by<=at.getY()+height+y){Entity entity=from(states,i);if(entity!=null)result.add(entity);}}return result;
 }
 public static List<org.bukkit.potion.PotionEffect> effects(String uuid){String[] states=Host.call(96,uuid);List<org.bukkit.potion.PotionEffect> result=new ArrayList<>();for(int i=0;i<states.length;i+=6){org.bukkit.potion.PotionEffectType type=org.bukkit.potion.PotionEffectType.getByName(states[i]);if(type!=null)result.add(new org.bukkit.potion.PotionEffect(type,Integer.parseInt(states[i+1]),Integer.parseInt(states[i+2]),states[i+3].equals("1"),states[i+4].equals("1"),states[i+5].equals("1")));}return result;}
 @Override public boolean equals(Object other){return other instanceof Entity&&getUniqueId().equals(((Entity)other).getUniqueId());}@Override public int hashCode(){return getUniqueId().hashCode();}
 private static class Living extends RemoteEntity implements LivingEntity {
  Living(String uuid){super(uuid);}public double getHealth(){return Double.parseDouble(snapshot()[10]);}public void setHealth(double value){Host.call(98,uuid,""+value);}
  public org.bukkit.attribute.AttributeInstance getAttribute(org.bukkit.attribute.Attribute type){if(type==org.bukkit.attribute.Attribute.MAX_HEALTH||type==org.bukkit.attribute.Attribute.ATTACK_DAMAGE||type==org.bukkit.attribute.Attribute.MOVEMENT_SPEED)return new RemoteAttribute(uuid,type);return null;}
  public Collection<org.bukkit.potion.PotionEffect> getActivePotionEffects(){return effects(uuid);}public org.bukkit.potion.PotionEffect getPotionEffect(org.bukkit.potion.PotionEffectType type){for(org.bukkit.potion.PotionEffect effect:getActivePotionEffects())if(effect.getType()==type)return effect;return null;}
  public boolean addPotionEffect(org.bukkit.potion.PotionEffect effect){return Host.yes(30,uuid,effect.getType().getKey().getKey(),""+effect.getDuration(),""+effect.getAmplifier(),""+effect.isAmbient(),""+effect.hasParticles(),""+effect.hasIcon());}public void removePotionEffect(org.bukkit.potion.PotionEffectType type){Host.call(31,uuid,type.getKey().getKey());}
  public double getEyeHeight(){return Double.parseDouble(snapshot()[9])*0.85;}public Location getEyeLocation(){return getLocation().add(0,getEyeHeight(),0);}
  public org.bukkit.inventory.EntityEquipment getEquipment(){return slot->{if(slot.ordinal()>5)throw new IllegalArgumentException("Ячейка экипировки отсутствует у сущности");String[] item=Host.call(97,uuid,""+slot.ordinal());return item.length==0?new org.bukkit.inventory.ItemStack(Material.AIR):org.bukkit.inventory.ItemStack.fromHost(Material.matchMaterial(item[0]),Integer.parseInt(item[1]),item[2]);};}
 }
 private static class Horse extends BasicMob implements AbstractHorse {Horse(String uuid){super(uuid);}public boolean isTamed(){return Host.call(104,uuid)[0].equals("1");}public void setTamed(boolean value){Host.call(106,uuid,"0",value?"1":"0");}public void setOwner(AnimalTamer owner){Host.call(106,uuid,"1",owner==null?"":owner.getUniqueId().toString());}public org.bukkit.inventory.AbstractHorseInventory getInventory(){return new HorseItems(this);}}
 private static final class Chested extends Horse implements ChestedHorse {Chested(String uuid){super(uuid);}public boolean isCarryingChest(){return Host.call(104,uuid)[1].equals("1");}}
 private static final class VexEntity extends BasicMob implements Vex,Enemy {VexEntity(String uuid){super(uuid);}}
 private static final class Stand extends Living implements ArmorStand {Stand(String uuid){super(uuid);}}
 private static class BasicMob extends Living implements Mob {BasicMob(String uuid){super(uuid);}}
 private static final class Animal extends BasicMob implements Animals {Animal(String uuid){super(uuid);}}
 private static final class Hostile extends BasicMob implements Enemy {Hostile(String uuid){super(uuid);}}
 private static final class AllayEntity extends BasicMob implements Allay {AllayEntity(String uuid){super(uuid);}}
 private static class Shot extends RemoteEntity implements Projectile {Shot(String uuid){super(uuid);}public org.bukkit.projectiles.ProjectileSource getShooter(){String shooter=snapshot()[14];if(shooter.isEmpty())return null;Entity entity=find(shooter);return entity instanceof org.bukkit.projectiles.ProjectileSource?(org.bukkit.projectiles.ProjectileSource)entity:null;}}
 private static final class Arrow extends Shot implements AbstractArrow {Arrow(String uuid){super(uuid);}public boolean isInBlock(){return snapshot()[15].equals("1");}public void setCritical(boolean value){Host.call(100,uuid,""+value);}}
 private static class Fire extends Shot implements Fireball {Fire(String uuid){super(uuid);}public void setYield(float value){Host.call(103,uuid,"0",""+value);}public void setIsIncendiary(boolean value){Host.call(103,uuid,"1",value?"1":"0");}}
 private static final class SmallFire extends Fire implements SmallFireball {SmallFire(String uuid){super(uuid);}}
 private static final class Wind extends Shot implements WindCharge {Wind(String uuid){super(uuid);}}
 private static final class Cloud extends RemoteEntity implements AreaEffectCloud {Cloud(String uuid){super(uuid);}public org.bukkit.potion.PotionType getBasePotionType(){String name=Host.text(114,uuid);return name.isEmpty()?null:org.bukkit.potion.PotionType.valueOf(Ascii.upper(name));}public List<org.bukkit.potion.PotionEffect> getCustomEffects(){return Collections.unmodifiableList(effects(uuid));}public org.bukkit.projectiles.ProjectileSource getSource(){String source=snapshot()[14];if(source.isEmpty())return null;Entity entity=find(source);return entity instanceof org.bukkit.projectiles.ProjectileSource?(org.bukkit.projectiles.ProjectileSource)entity:null;}}
 private static final class Rocket extends Shot implements Firework {Rocket(String uuid){super(uuid);}}
 private static final class Frame extends RemoteEntity implements ItemFrame {Frame(String uuid){super(uuid);}}
 private static final class Potion extends Shot implements ThrownPotion {Potion(String uuid){super(uuid);}public Collection<org.bukkit.potion.PotionEffect> getEffects(){return effects(uuid);}public void setItem(org.bukkit.inventory.ItemStack stack){Host.call(101,uuid,Ascii.lower(stack.getType().name()),""+stack.getAmount(),stack.metadata());}}
}
