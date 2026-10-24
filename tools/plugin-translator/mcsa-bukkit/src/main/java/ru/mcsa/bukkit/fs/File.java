package ru.mcsa.bukkit.fs;
import ru.mcsa.bukkit.Host;
public final class File {
 static final String ROOT="__plugin_data__";final String name;
 public File(String path){if(!path.equals(ROOT)&&!path.startsWith(ROOT+"/"))throw new SecurityException("Доступ разрешён только к папке данных плагина");name=check(path.equals(ROOT)?"":path.substring(ROOT.length()+1));}
 public File(File parent,String child){if(parent==null)throw new SecurityException("Нет папки данных");name=check((parent.name.isEmpty()?"":parent.name+"/")+child);}
 static String check(String name){if(name.length()>512)throw new SecurityException("Длинный путь");if(!name.isEmpty())for(String part:ru.mcsa.bukkit.Strings.split(name,'/')){if(part.isEmpty()||part.length()>100||part.charAt(0)=='.')throw new SecurityException("Недопустимый путь в папке данных");for(int i=0;i<part.length();i++){char c=part.charAt(i);if(!(c>='A'&&c<='Z'||c>='a'&&c<='z'||c>='0'&&c<='9'||c=='_'||c=='-'||c=='.'))throw new SecurityException("Недопустимый путь в папке данных");}}return name;}

 public String getName(){return name.substring(name.lastIndexOf('/')+1);}public String getPath(){return ROOT+(name.isEmpty()?"":"/"+name);}public String toString(){return getPath();}
 public boolean exists(){return !name.isEmpty()&&Host.yes(34,name);}public boolean isFile(){return exists();}
 public boolean renameTo(File target){try{Host.call(36,name,target.name,"true","true");return true;}catch(IllegalStateException e){return false;}}
 public boolean delete(){return Host.yes(35,name);}public Path toPath(){return new DataPath(name);}
}
