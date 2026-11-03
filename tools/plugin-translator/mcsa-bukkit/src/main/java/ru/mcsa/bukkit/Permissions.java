package ru.mcsa.bukkit;
import java.util.*;
// Значения берутся только из проверенного plugin.yml; права оператора сообщает хозяин.
public final class Permissions {
 private static final Map<String,String> defaults=new LinkedHashMap<>();
 private static final Map<String,Map<String,Boolean>> children=new LinkedHashMap<>();
 public static void declare(String name,String value){defaults.put(key(name),value);}
 public static void child(String parent,String name,boolean value){children.computeIfAbsent(key(parent),k->new LinkedHashMap<>()).put(key(name),value);}
 public static String key(String value){return Ascii.lower(value);}

 public static boolean has(String permission,boolean op){
  Map<String,Boolean> values=new LinkedHashMap<>();
  for(Map.Entry<String,String> entry:defaults.entrySet())if(value(entry.getValue(),op))expand(entry.getKey(),false,values,new HashSet<>());
  String name=key(permission);return values.containsKey(name)?values.get(name):value(defaults.getOrDefault(name,"op"),op);
 }
 private static boolean value(String value,boolean op){return value.equals("true")||value.equals("op")&&op||value.equals("not op")&&!op;}
 private static void expand(String name,boolean invert,Map<String,Boolean> values,Set<String> path){
  if(!path.add(name+":"+invert))return;values.put(name,!invert);
  for(Map.Entry<String,Boolean> child:children.getOrDefault(name,Collections.emptyMap()).entrySet())expand(child.getKey(),invert^!child.getValue(),values,path);

 }
}
