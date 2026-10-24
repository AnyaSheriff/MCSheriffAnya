package ru.mcsa.bukkit;
// Разделители путей и формата не требуют запуска регулярных выражений в Wasm.
public final class Strings {
 public static String[] split(String value,char separator){java.util.List<String> out=new java.util.ArrayList<>();int from=0;for(int i=0;i<=value.length();i++)if(i==value.length()||value.charAt(i)==separator){out.add(value.substring(from,i));from=i+1;}return out.toArray(new String[0]);}
}
