package ru.mcsa.bukkit;
// Имена материалов, ключей и полей протокола состоят из ASCII, а не из текста чата.
public final class Ascii {
 public static String lower(String value){return convert(value,false);}public static String upper(String value){return convert(value,true);}
 private static String convert(String value,boolean upper){StringBuilder result=new StringBuilder();for(int i=0;i<value.length();i++){char c=value.charAt(i);if(upper&&c>='a'&&c<='z')c=(char)(c-32);else if(!upper&&c>='A'&&c<='Z')c=(char)(c+32);result.append(c);}return result.toString();}
}
