package ru.mcsa.bukkit;
// Ограниченный снимок для последовательных чтений; любой иной вызов хозяина сбрасывает его.
public final class BlockReads {
 private static long epoch=-1;private static int reads;private static final java.util.Map<String,String[]> cubes=new java.util.HashMap<>();
 public static void hostCall(int op){if(epoch!=RuntimeState.callbackEpoch||op!=13&&op!=161){epoch=RuntimeState.callbackEpoch;reads=0;cubes.clear();}}
 public static String type(int dimension,int x,int y,int z){
  hostCall(13);if(++reads<16||y< -2048||y>2040)return Host.text(13,""+dimension,""+x,""+y,""+z);
  int bx=x&~7,by=y&~7,bz=z&~7;String key=dimension+":"+bx+":"+by+":"+bz;String[] cube=cubes.get(key);
  if(cube==null){cube=Host.call(161,""+dimension,""+bx,""+by,""+bz);if(cube.length!=512)throw new IllegalStateException("Неполный куб блоков");if(cubes.size()>=64)cubes.clear();cubes.put(key,cube);}
  String name=cube[((x-bx)*8+y-by)*8+z-bz];return name.isEmpty()?Host.text(13,""+dimension,""+x,""+y,""+z):name;
 }
}
