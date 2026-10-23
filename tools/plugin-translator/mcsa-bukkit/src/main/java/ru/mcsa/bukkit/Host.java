package ru.mcsa.bukkit;
import java.nio.charset.StandardCharsets;
import org.teavm.interop.*;
public final class Host {
 @Import(name="mcsa_call") private static native Address invoke(int op, Address bytes, int length);
 @Import(name="mcsa_length") private static native int length();
 public static String[] call(int op, String... args) {
  StringBuilder b=new StringBuilder(); for(String a:args){if(a.indexOf('\0')>=0)throw new IllegalArgumentException("Нулевой символ запрещён"); b.append(a).append('\0');}
  byte[] bytes=b.toString().getBytes(StandardCharsets.UTF_8); Address out=invoke(op,Address.ofData(bytes),bytes.length); Address.pin(bytes);
  byte[] result=new byte[length()]; for(int i=0;i<result.length;i++)result[i]=out.add(i).getByte();
  String[] fields=split(new String(result,StandardCharsets.UTF_8));
  if(fields.length>0&&fields[0].equals("!"))throw new IllegalStateException(fields.length>1?fields[1]:"Ошибка хозяина"); return fields;
 }
 public static String[] split(String s){java.util.List<String> r=new java.util.ArrayList<>();int start=0;for(int i=0;i<s.length();i++)if(s.charAt(i)==0){r.add(s.substring(start,i));start=i+1;}return r.toArray(new String[0]);}
 public static String text(int op,String... a){String[] r=call(op,a);return r.length==0?"":r[0];}
 public static boolean yes(int op,String... a){return text(op,a).equals("1");}
 public static org.bukkit.entity.Player player(String[] a,int i){return new RemotePlayer(a,i);}
}
