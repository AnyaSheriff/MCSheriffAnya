package ru.mcsa.bukkit;
import org.bukkit.*;
public final class RemoteBlock implements org.bukkit.block.Block { private final World world; private final int x,y,z;private String proposed;public RemoteBlock(World w,int x,int y,int z){world=w;this.x=x;this.y=y;this.z=z;}
 public RemoteBlock(World w,int x,int y,int z,String proposed){this(w,x,y,z);this.proposed=proposed;}
 public World getWorld(){return world;} public int getX(){return x;}public int getY(){return y;}public int getZ(){return z;} public Location getLocation(){return new Location(world,x,y,z);}
 public String[] arguments(String... extra){String[] result=java.util.Arrays.copyOf(args(),4+extra.length);System.arraycopy(extra,0,result,4,extra.length);return result;}
 private String[] details(){return BlockReads.details(((RemoteWorld)world).dimension,x,y,z);}
 public boolean breakNaturally(){return Host.yes(87,args());}
 public byte getLightFromSky(){return Byte.parseByte(details()[1]);}public byte getLightLevel(){return Byte.parseByte(details()[2]);}
 public boolean isLiquid(){Material type=getType();return type==Material.WATER||type==Material.LAVA;}public boolean isPassable(){Material type=getType();return type.isAir()||type==Material.WATER||type==Material.LAVA||details()[4].equals("1");}
 public void randomTick(){Host.call(71,args());}public org.bukkit.block.BlockState getState(){return new BlockSnapshot(this,details());}
 private String[] args(){return new String[]{""+((RemoteWorld)world).dimension,""+x,""+y,""+z};}
 public Material getType(){String name=proposed==null?BlockReads.type(((RemoteWorld)world).dimension,x,y,z):proposed;Material m=Material.matchMaterial(name);if(m==null)throw new IllegalStateException("Материал вне подмножества: "+name);return m;}
 public void setType(Material m){String[] a=args();Host.call(14,a[0],a[1],a[2],a[3],Ascii.lower(m.name()));}
}
