package org.bukkit;
public class Location implements Cloneable {
 private World world;private double x,y,z;private float yaw,pitch;
 public Location(World w,double x,double y,double z){this(w,x,y,z,0,0);} public Location(World w,double x,double y,double z,float yaw,float pitch){world=w;this.x=x;this.y=y;this.z=z;this.yaw=yaw;this.pitch=pitch;}
 public World getWorld(){return world;} public double getX(){return x;} public double getY(){return y;} public double getZ(){return z;}
 public int getBlockX(){return (int)Math.floor(x);}public int getBlockY(){return (int)Math.floor(y);}public int getBlockZ(){return (int)Math.floor(z);}
 public float getYaw(){return yaw;} public float getPitch(){return pitch;} public void setX(double v){x=v;}public void setY(double v){y=v;}public void setZ(double v){z=v;}
 public Location add(double a,double b,double c){x+=a;y+=b;z+=c;return this;} public Location clone(){return new Location(world,x,y,z,yaw,pitch);}
 public org.bukkit.block.Block getBlock(){return world.getBlockAt(getBlockX(),getBlockY(),getBlockZ());}
}
