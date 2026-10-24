package org.bukkit;
public class Location implements Cloneable {
 private World world;private double x,y,z;private float yaw,pitch;
 public Location(World w,double x,double y,double z){this(w,x,y,z,0,0);} public Location(World w,double x,double y,double z,float yaw,float pitch){world=w;this.x=x;this.y=y;this.z=z;this.yaw=yaw;this.pitch=pitch;}
 public World getWorld(){return world;} public double getX(){return x;} public double getY(){return y;} public double getZ(){return z;}
 public int getBlockX(){return (int)Math.floor(x);}public int getBlockY(){return (int)Math.floor(y);}public int getBlockZ(){return (int)Math.floor(z);}
 public float getYaw(){return yaw;} public float getPitch(){return pitch;} public void setX(double v){x=v;}public void setY(double v){y=v;}public void setZ(double v){z=v;}
 public Location add(double a,double b,double c){x+=a;y+=b;z+=c;return this;} public Location clone(){return new Location(world,x,y,z,yaw,pitch);}
 public void setYaw(float yaw){this.yaw=yaw;}public void setPitch(float pitch){this.pitch=pitch;}
 public org.bukkit.util.Vector toVector(){return new org.bukkit.util.Vector(x,y,z);}public Location add(org.bukkit.util.Vector v){return add(v.getX(),v.getY(),v.getZ());}
 public org.bukkit.util.Vector getDirection(){double p=Math.toRadians(pitch),a=Math.toRadians(yaw),h=Math.cos(p);return new org.bukkit.util.Vector(-h*Math.sin(a),-Math.sin(p),h*Math.cos(a));}
 public double distanceSquared(Location other){if(world==null||other.world==null||!world.getName().equals(other.world.getName()))throw new IllegalArgumentException("Разные миры");double a=x-other.x,b=y-other.y,c=z-other.z;return a*a+b*b+c*c;}public double distance(Location other){return Math.sqrt(distanceSquared(other));}
 public java.util.Collection<org.bukkit.entity.Player> getNearbyPlayers(double radius){if(!Double.isFinite(radius)||radius<0)throw new IllegalArgumentException("Неверный радиус");java.util.List<org.bukkit.entity.Player> result=new java.util.ArrayList<>();for(org.bukkit.entity.Player p:Bukkit.getOnlinePlayers()){Location l=p.getLocation();if(world.getName().equals(l.world.getName())&&Math.abs(x-l.x)<=radius&&Math.abs(y-l.y)<=radius&&Math.abs(z-l.z)<=radius)result.add(p);}return result;}
 public org.bukkit.block.Block getBlock(){return world.getBlockAt(getBlockX(),getBlockY(),getBlockZ());}
}
