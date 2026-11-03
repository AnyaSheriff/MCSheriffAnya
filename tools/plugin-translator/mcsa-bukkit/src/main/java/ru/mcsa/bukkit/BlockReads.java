package ru.mcsa.bukkit;
// Ограниченный снимок для последовательных чтений; любой иной вызов хозяина сбрасывает его.
public final class BlockReads {
 private static long epoch=-1;private static int reads;private static int lastDimension,lastX,lastY,lastZ;private static String[] lastCube;private static final java.util.Map<String,String[]> cubes=new java.util.HashMap<>(),details=new java.util.HashMap<>();
 public static void hostCall(int op){if(epoch!=RuntimeState.callbackEpoch||op!=13&&op!=161&&op!=69){epoch=RuntimeState.callbackEpoch;reads=0;cubes.clear();details.clear();lastCube=null;}}
 // Метаданные тоже читаются согласованно до следующего изменяющего вызова.
 public static String[] details(int dimension,int x,int y,int z){hostCall(69);String key=dimension+":"+x+":"+y+":"+z;String[] value=details.get(key);if(value==null){value=Host.call(69,""+dimension,""+x,""+y,""+z);if(details.size()>=512)details.clear();details.put(key,value);}return value;}
 public static String type(int dimension,int x,int y,int z){
  hostCall(13);if(++reads<16||y< -2048||y>2040)return Host.text(13,""+dimension,""+x,""+y,""+z);
  int bx=x&~7,by=y&~7,bz=z&~7;String[] cube=lastCube;
  if(cube==null||dimension!=lastDimension||bx!=lastX||by!=lastY||bz!=lastZ){String key=dimension+":"+bx+":"+by+":"+bz;cube=cubes.get(key);
  if(cube==null){cube=decode(Host.call(161,""+dimension,""+bx,""+by,""+bz));if(cubes.size()>=64)cubes.clear();cubes.put(key,cube);}lastDimension=dimension;lastX=bx;lastY=by;lastZ=bz;lastCube=cube;}
  String name=cube[((x-bx)*8+y-by)*8+z-bz];return name.isEmpty()?Host.text(13,""+dimension,""+x,""+y,""+z):name;
 }
 // Снимок остаётся прежним массивом 8³, но повторные имена разделяют одну строку.
 public static String[] decode(String[] runs){String[] cube=new String[512];int at=0;if(runs.length%2!=0)throw new IllegalStateException("Неполный куб блоков");for(int i=0;i<runs.length;i+=2){int count=Integer.parseInt(runs[i]);if(count<=0||count>512-at)throw new IllegalStateException("Неверная серия блоков");java.util.Arrays.fill(cube,at,at+count,runs[i+1]);at+=count;}if(at!=512)throw new IllegalStateException("Неполный куб блоков");return cube;}
}
