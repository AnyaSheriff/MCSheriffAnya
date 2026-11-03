package ru.mcsa.bukkit;
import org.bukkit.Material;import org.bukkit.generator.*;
// Область принадлежит одному вызову: доступ после возврата запрещён.
public final class Population implements LimitedRegion {
 private static final java.util.List<java.util.List<BlockPopulator>> populators=new java.util.ArrayList<>();
 static {for(int i=0;i<3;i++)populators.add(new java.util.ArrayList<>());}
 public static java.util.List<BlockPopulator> list(int dimension){return populators.get(dimension);}
 public static boolean wanted(int dimension){return !list(dimension).isEmpty();}
 private final int cx,cz;private final Material[] blocks=new Material[98304];private final boolean[] changed=new boolean[98304];private boolean active=true;
 private Population(String[] a){cx=Integer.parseInt(a[1]);cz=Integer.parseInt(a[2]);int count=Integer.parseInt(a[4]);Material[] palette=new Material[count];for(int i=0;i<count;i++){palette[i]=Material.matchMaterial(a[5+i]);if(palette[i]==null)throw new IllegalStateException("Неизвестный материал области: "+a[5+i]);}if(a.length!=6+count)throw new IllegalStateException("Неверная область генерации");decode(a[5+count],palette);}
 // Читаем упакованные числа без split и отдельных строк на каждое число.
 private void decode(String runs,Material[] palette){int at=0,size=-1,value=0,digits=0;for(int i=0;i<runs.length();i++){char c=runs.charAt(i);if(c>='0'&&c<='9'){value=value*10+c-'0';if(value>98304)throw new IllegalStateException("Неверное число серии");digits++;}else if(c==','&&digits>0){if(size<0){size=value;if(size<1||at+size>blocks.length)throw new IllegalStateException("Неверная область генерации");}else{if(value>=palette.length)throw new IllegalStateException("Неверная палитра генерации");java.util.Arrays.fill(blocks,at,at+size,palette[value]);at+=size;size=-1;}value=0;digits=0;}else throw new IllegalStateException("Неверная серия генерации");}if(at!=blocks.length||size>=0||digits!=0)throw new IllegalStateException("Неполная область генерации");}

 private int index(int x,int y,int z){if(!active)throw new IllegalStateException("Область генерации уже закрыта");if(!isInRegion(x,y,z))throw new IllegalArgumentException("Вне области генерации: доступен только центральный чанк, buffer=0");return ((x-cx*16)*16+z-cz*16)*384+y+64;}
 public boolean isInRegion(int x,int y,int z){return x>=cx*16&&x<cx*16+16&&z>=cz*16&&z<cz*16+16&&y>=-64&&y<320;}public int getBuffer(){return 0;}
 public Material getType(int x,int y,int z){return blocks[index(x,y,z)];}public void setType(int x,int y,int z,Material type){int i=index(x,y,z);if(type==null)throw new IllegalArgumentException("Нужен материал блока");blocks[i]=type;changed[i]=true;}
 // Большие массивы области убираем в конечном бюджете генерации, до обычных callback.
 public static void run(String[] a){Host.call(22,apply(a));System.gc();}
 public static String apply(String[] a){Population region=new Population(a);int dimension=Integer.parseInt(a[0]);long seed=Long.parseLong(a[3]);try{for(BlockPopulator populator:new java.util.ArrayList<>(list(dimension)))populator.populate(new RemoteWorld(dimension),new java.util.Random(seed),region.cx,region.cz,region);StringBuilder edits=new StringBuilder();for(int i=0;i<region.blocks.length;i++)if(region.changed[i])edits.append(i).append(' ').append(Ascii.lower(region.blocks[i].name())).append('\n');return edits.toString();}finally{region.active=false;}}
}
