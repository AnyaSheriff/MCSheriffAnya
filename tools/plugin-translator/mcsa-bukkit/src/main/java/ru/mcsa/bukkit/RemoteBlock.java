package ru.mcsa.bukkit;
import org.bukkit.*;
public final class RemoteBlock implements org.bukkit.block.Block { private final World world; private final int x,y,z;private String proposed;public RemoteBlock(World w,int x,int y,int z){world=w;this.x=x;this.y=y;this.z=z;}
 public RemoteBlock(World w,int x,int y,int z,String proposed){this(w,x,y,z);this.proposed=proposed;}
 public World getWorld(){return world;} public int getX(){return x;}public int getY(){return y;}public int getZ(){return z;} public Location getLocation(){return new Location(world,x,y,z);}
 private String[] args(){return new String[]{""+((RemoteWorld)world).dimension,""+x,""+y,""+z};}
 public Material getType(){String name=proposed==null?Host.text(13,args()):proposed;Material m=Material.matchMaterial(name);if(m==null)throw new IllegalStateException("Материал вне подмножества: "+name);return m;}
 public void setType(Material m){String[] a=args();Host.call(14,a[0],a[1],a[2],a[3],m.name().toLowerCase(java.util.Locale.ROOT));}
}
