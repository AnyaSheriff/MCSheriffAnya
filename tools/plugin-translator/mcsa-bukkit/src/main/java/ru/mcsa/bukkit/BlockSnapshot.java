package ru.mcsa.bukkit;
import org.bukkit.*;import org.bukkit.block.*;import org.bukkit.block.data.*;
public final class BlockSnapshot implements BlockState {
 private final RemoteBlock block;private final String data;private final Material material;private final int age,maxAge;
 public BlockSnapshot(RemoteBlock block,String[] fields){this.block=block;data=fields[0];material=Material.matchMaterial(data.split("\\[")[0]);age=Integer.parseInt(fields[5]);maxAge=Integer.parseInt(fields[6]);}
 public Material getType(){return material;}
 public BlockData getBlockData(){return age<0?new Plain(material,data):new Aged(material,data,age,maxAge);}
 public boolean update(boolean force,boolean physics){return Host.yes(70,block.arguments(data,""+force,""+physics));}
 private static class Plain implements BlockData {private final Material material;private final String data;Plain(Material m,String s){material=m;data=s;}public Material getMaterial(){return material;}public String getAsString(){return data;}}
 private static final class Aged extends Plain implements Ageable {private final int age,max;Aged(Material m,String s,int age,int max){super(m,s);this.age=age;this.max=max;}public int getAge(){return age;}public int getMaximumAge(){return max;}}
}
