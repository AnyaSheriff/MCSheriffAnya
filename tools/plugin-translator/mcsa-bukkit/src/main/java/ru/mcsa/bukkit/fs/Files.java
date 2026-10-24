package ru.mcsa.bukkit.fs;
import java.io.IOException;
import java.nio.charset.Charset;
import ru.mcsa.bukkit.Host;
public final class Files {
 private static String name(Path path){if(!(path instanceof DataPath))throw new SecurityException("Чужой путь");return ((DataPath)path).name;}
 public static boolean exists(Path path,LinkOption... options){return path.toFile().exists();}
 public static Path createDirectories(Path path,FileAttribute<?>... attributes)throws IOException{if(attributes.length!=0)throw new UnsupportedOperationException("Атрибуты файлов недоступны");name(path);return path;}
 public static Path writeString(Path path,CharSequence text,Charset charset,OpenOption... options)throws IOException{if(!charset.equals(java.nio.charset.StandardCharsets.UTF_8)||options.length!=0)throw new UnsupportedOperationException("Только запись UTF-8");try{Host.call(9,name(path),text.toString());return path;}catch(IllegalStateException e){throw new IOException(e.getMessage());}}
 private static Path transfer(Path source,Path target,boolean move,CopyOption... options)throws IOException{boolean replace=false;for(CopyOption option:options){if(option==StandardCopyOption.REPLACE_EXISTING)replace=true;else if(option!=StandardCopyOption.ATOMIC_MOVE||!move)throw new UnsupportedOperationException("Неподдерживаемая опция файла");}try{Host.call(36,name(source),name(target),""+replace,""+move);return target;}catch(IllegalStateException e){throw new IOException(e.getMessage());}}
 public static Path copy(Path source,Path target,CopyOption... options)throws IOException{return transfer(source,target,false,options);}
 public static Path move(Path source,Path target,CopyOption... options)throws IOException{return transfer(source,target,true,options);}
}
