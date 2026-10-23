package ru.mcsa.bukkit;
import org.bukkit.*;
public final class RemoteBorder implements WorldBorder {
 private final RemoteWorld world;public RemoteBorder(RemoteWorld world){this.world=world;}
 private double[] read(){String[] values=Host.call(32,""+world.dimension);return new double[]{Double.parseDouble(values[0]),Double.parseDouble(values[1]),Double.parseDouble(values[2])};}
 private void write(double size,double x,double z){Host.call(33,""+world.dimension,""+size,""+x,""+z);}
 public World getWorld(){return world;}public double getSize(){return read()[0];}
 public Location getCenter(){double[] value=read();return new Location(world,value[1],0,value[2]);}
 public void setSize(double size){double[] value=read();write(size,value[1],value[2]);}
 public void setCenter(double x,double z){write(getSize(),x,z);}public void setCenter(Location center){setCenter(center.getX(),center.getZ());}
}
