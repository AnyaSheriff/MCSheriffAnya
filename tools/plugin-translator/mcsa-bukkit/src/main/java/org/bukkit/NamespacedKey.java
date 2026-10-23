package org.bukkit;
import java.util.*;
public final class NamespacedKey implements net.kyori.adventure.key.Key {
 public static final String MINECRAFT="minecraft",BUKKIT="bukkit";
 private final String namespace,key;
 private static boolean valid(String value,boolean path){if(value==null||value.isEmpty())return false;for(int i=0;i<value.length();i++){char c=value.charAt(i);if(!((c>='a'&&c<='z')||(c>='0'&&c<='9')||c=='.'||c=='_'||c=='-'||(path&&c=='/')))return false;}return true;}
 public NamespacedKey(String namespace,String key){if(!valid(namespace,false)||!valid(key,true))throw new IllegalArgumentException("Недопустимый ключ: "+namespace+":"+key);this.namespace=namespace;this.key=key;}
 public NamespacedKey(org.bukkit.plugin.Plugin plugin,String key){this(plugin.getName().toLowerCase(Locale.ROOT),key);}
 public String getNamespace(){return namespace;} public String getKey(){return key;} public String namespace(){return namespace;}public String value(){return key;}
 public static NamespacedKey minecraft(String key){return new NamespacedKey(MINECRAFT,key);}
 public static NamespacedKey fromString(String value){return fromString(value,null);}
 public static NamespacedKey fromString(String value,org.bukkit.plugin.Plugin plugin){if(value==null||value.isEmpty())return null;int colon=value.indexOf(':');try{return colon<0?new NamespacedKey(plugin==null?MINECRAFT:plugin.getName().toLowerCase(Locale.ROOT),value):new NamespacedKey(colon==0?MINECRAFT:value.substring(0,colon),value.substring(colon+1));}catch(IllegalArgumentException e){return null;}}
 public String toString(){return asString();} public boolean equals(Object o){return o instanceof NamespacedKey&&namespace.equals(((NamespacedKey)o).namespace)&&key.equals(((NamespacedKey)o).key);}public int hashCode(){return Objects.hash(namespace,key);}
}
