package ru.mcsa.bukkit;
import org.bukkit.*;
public final class RemoteWorld implements World {
 @Override public boolean equals(Object value){return value instanceof RemoteWorld&&dimension==((RemoteWorld)value).dimension;}@Override public int hashCode(){return dimension;}
 public java.util.List<org.bukkit.generator.BlockPopulator> getPopulators(){return Population.list(dimension);}
 public final int dimension; public RemoteWorld(int d){dimension=d;} public String getName(){return dimension==0?"world":dimension==1?"world_nether":"world_the_end";}
 private org.bukkit.entity.Item drop(Location loc,org.bukkit.inventory.ItemStack item,boolean natural){java.util.Objects.requireNonNull(item);if(loc.getWorld()==null||!loc.getWorld().getName().equals(getName()))throw new IllegalArgumentException("Разные миры");return new RemoteItem(Host.text(79,""+dimension,""+loc.getX(),""+loc.getY(),""+loc.getZ(),Ascii.lower(item.getType().name()),""+item.getAmount(),item.metadata(),""+natural));}
 public org.bukkit.entity.Item dropItem(Location loc,org.bukkit.inventory.ItemStack item){return drop(loc,item,false);}public org.bukkit.entity.Item dropItemNaturally(Location loc,org.bukkit.inventory.ItemStack item){return drop(loc,item,true);}
 public void spawnParticle(Particle particle,Location loc,int count,double dx,double dy,double dz){if(loc.getWorld()==null||!loc.getWorld().getName().equals(getName()))throw new IllegalArgumentException("Разные миры");Host.call(86,""+dimension,""+loc.getX(),""+loc.getY(),""+loc.getZ(),Ascii.lower(particle.name()),""+count,""+dx,""+dy,""+dz);}
 public boolean isChunkLoaded(int x,int z){return Host.yes(72,""+dimension,""+x,""+z);}
 public int getHighestBlockYAt(Location loc){if(loc.getWorld()!=null&&!loc.getWorld().getName().equals(getName()))throw new IllegalArgumentException("Разные миры");return Integer.parseInt(Host.text(73,""+dimension,""+loc.getBlockX(),""+loc.getBlockZ()));}
 @SuppressWarnings("unchecked") public <T>T getGameRuleValue(GameRule<T> rule){if(rule!=GameRule.RANDOM_TICK_SPEED)throw new UnsupportedOperationException("Неизвестное правило мира");return (T)Integer.valueOf(Host.text(74));}
 public void playSound(Location loc,Sound sound,float volume,float pitch){if(loc.getWorld()==null||!loc.getWorld().getName().equals(getName()))throw new IllegalArgumentException("Разные миры");Host.call(75,""+dimension,""+loc.getX(),""+loc.getY(),""+loc.getZ(),sound.getKey().toString(),""+volume,""+pitch);}
 public Environment getEnvironment(){return dimension==0?Environment.NORMAL:dimension==1?Environment.NETHER:Environment.THE_END;}
 public int getMinHeight(){return -64;}public int getMaxHeight(){return 320;}
 public java.util.List<org.bukkit.entity.Player> getPlayers(){java.util.List<org.bukkit.entity.Player> result=new java.util.ArrayList<>();for(org.bukkit.entity.Player player:Bukkit.getOnlinePlayers())if(((RemotePlayer)player).snapshotLocation().getWorld().getName().equals(getName()))result.add(player);return result;}
 public org.bukkit.block.Block getBlockAt(int x,int y,int z){return new RemoteBlock(this,x,y,z);}
 public org.bukkit.util.RayTraceResult rayTraceBlocks(Location start,org.bukkit.util.Vector direction,double maxDistance,FluidCollisionMode fluids,boolean ignorePassable){
  if(start.getWorld()==null||!start.getWorld().getName().equals(getName()))throw new IllegalArgumentException("Разные миры");java.util.Objects.requireNonNull(direction);java.util.Objects.requireNonNull(fluids);
  if(maxDistance<0)return null;
  String[] hit=Host.call(91,""+dimension,""+start.getX(),""+start.getY(),""+start.getZ(),""+direction.getX(),""+direction.getY(),""+direction.getZ(),""+maxDistance,""+fluids.ordinal(),""+ignorePassable);
  if(hit.length==0)return null;return new org.bukkit.util.RayTraceResult(new org.bukkit.util.Vector(Double.parseDouble(hit[0]),Double.parseDouble(hit[1]),Double.parseDouble(hit[2])),getBlockAt(Integer.parseInt(hit[3]),Integer.parseInt(hit[4]),Integer.parseInt(hit[5])),org.bukkit.block.BlockFace.values()[Integer.parseInt(hit[6])]);
 }
 public int getSpawnLimit(org.bukkit.entity.SpawnCategory category){java.util.Objects.requireNonNull(category);return Integer.parseInt(Host.text(92,""+dimension));}
 public void setSpawnLimit(org.bukkit.entity.SpawnCategory category,int limit){java.util.Objects.requireNonNull(category);Host.call(93,""+dimension,""+limit);}
 public <T extends org.bukkit.entity.Entity> java.util.Collection<T> getEntitiesByClass(Class<T> type){java.util.Objects.requireNonNull(type);java.util.List<T> result=new java.util.ArrayList<>();for(org.bukkit.entity.Entity entity:RemoteEntity.world(this))if(type.isInstance(entity))result.add(type.cast(entity));return result;}
 public org.bukkit.util.RayTraceResult rayTrace(Location start,org.bukkit.util.Vector direction,double maxDistance,FluidCollisionMode fluids,boolean ignorePassable,double raySize,java.util.function.Predicate<? super org.bukkit.entity.Entity> filter){
  if(!Double.isFinite(raySize)||Math.abs(raySize)>64)throw new IllegalArgumentException("Размер луча должен быть конечным, по модулю до 64");
  org.bukkit.util.RayTraceResult block=rayTraceBlocks(start,direction,maxDistance,fluids,ignorePassable);if(maxDistance<0)return null;double length=direction.length();org.bukkit.util.Vector dir=direction.clone().multiply(1/length);double nearest=block==null?maxDistance:block.getHitPosition().subtract(start.toVector()).length();org.bukkit.util.RayTraceResult result=block;
  String[] states=Host.call(95,""+dimension);for(int i=0;i<states.length;i+=16){org.bukkit.entity.Entity entity=RemoteEntity.from(states,i);if(entity==null||filter!=null&&!filter.test(entity))continue;
   double x=Double.parseDouble(states[i+3]),y=Double.parseDouble(states[i+4]),z=Double.parseDouble(states[i+5]),width=Double.parseDouble(states[i+8]),height=Double.parseDouble(states[i+9]);double[] low={x-width/2-raySize,y-raySize,z-width/2-raySize},high={x+width/2+raySize,y+height+raySize,z+width/2+raySize},origin={start.getX(),start.getY(),start.getZ()},step={dir.getX(),dir.getY(),dir.getZ()};double near=0,far=nearest;
   for(int axis=0;axis<3;axis++){if(low[axis]>high[axis]){far=-1;break;}if(step[axis]==0){if(origin[axis]<low[axis]||origin[axis]>high[axis]){far=-1;break;}}else {double a=(low[axis]-origin[axis])/step[axis],b=(high[axis]-origin[axis])/step[axis];near=Math.max(near,Math.min(a,b));far=Math.min(far,Math.max(a,b));}}
   if(near<=far&&near<nearest){nearest=near;result=new org.bukkit.util.RayTraceResult(start.toVector().add(dir.clone().multiply(near)),entity);}
  }return result;
 }
 public <T extends org.bukkit.entity.Entity> T spawn(Location at,Class<T> type,java.util.function.Consumer<? super T> configure){
  if(type!=org.bukkit.entity.Vex.class)throw new UnsupportedOperationException("Создание этого класса через Bukkit пока не поддерживается");if(at.getWorld()==null||!at.getWorld().getName().equals(getName()))throw new IllegalArgumentException("Разные миры");
  String uuid=Host.text(107,""+dimension,""+at.getX(),""+at.getY(),""+at.getZ(),""+at.getYaw(),""+at.getPitch(),"vex");boolean done=false;
  try{T entity=type.cast(RemoteEntity.find(uuid));if(configure!=null)configure.accept(entity);Host.call(108,uuid,"1");done=true;return entity;}finally{if(!done)Host.call(108,uuid,"0");}
 }
 public WorldBorder getWorldBorder(){return new RemoteBorder(this);}
 public Location getSpawnLocation(){String[] r=Host.call(15,""+dimension);return new Location(this,Double.parseDouble(r[0]),Double.parseDouble(r[1]),Double.parseDouble(r[2]));}
 public long getTime(){return Long.parseLong(Host.text(16,""+dimension));}public void setTime(long t){if(t<0)throw new IllegalArgumentException("Время меньше нуля");Host.call(17,""+dimension,""+(t%24000));}
 public boolean hasStorm(){return !Host.text(18,""+dimension).equals("clear");}public boolean isThundering(){return Host.text(18,""+dimension).equals("thunder");}
 public void setStorm(boolean b){Host.call(19,""+dimension,b?"rain":"clear");}public void setThundering(boolean b){Host.call(19,""+dimension,b?"thunder":"clear");}
}
