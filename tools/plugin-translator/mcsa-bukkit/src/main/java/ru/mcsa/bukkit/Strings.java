package ru.mcsa.bukkit;
// Разделители путей и формата не требуют запуска регулярных выражений в Wasm.
public final class Strings {
 // ASCII-пути Locale.ROOT не требуют холодной загрузки таблиц Unicode.
 public static String lower(String value,java.util.Locale locale){java.util.Objects.requireNonNull(locale);if(locale==java.util.Locale.ROOT&&ascii(value)){String result=Ascii.lower(value);return result.equals(value)?value:result;}return value.toLowerCase(locale);}
 public static String upper(String value,java.util.Locale locale){java.util.Objects.requireNonNull(locale);if(locale==java.util.Locale.ROOT&&ascii(value)){String result=Ascii.upper(value);return result.equals(value)?value:result;}return value.toUpperCase(locale);}
 private static boolean ascii(String value){for(int i=0;i<value.length();i++)if(value.charAt(i)>127)return false;return true;}

 public static String[] split(String value,char separator){java.util.List<String> out=new java.util.ArrayList<>();int from=0;for(int i=0;i<=value.length();i++)if(i==value.length()||value.charAt(i)==separator){out.add(value.substring(from,i));from=i+1;}return out.toArray(new String[0]);}
}
