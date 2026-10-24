package org.mcsa.api;
// Запрос исполняет ядро. Плагин опрашивает результат между тактами.
public final class Https {
 private static String hex(byte[] data){StringBuilder out=new StringBuilder();for(byte value:data){out.append("0123456789abcdef".charAt((value&255)>>>4));out.append("0123456789abcdef".charAt(value&15));}return out.toString();}
 private static byte[] unhex(String text){if(text.length()%2!=0)throw new IllegalStateException("Неверный ответ хозяина");byte[] out=new byte[text.length()/2];for(int i=0;i<out.length;i++)out[i]=(byte)Integer.parseInt(text.substring(i*2,i*2+2),16);return out;}
 public static int get(String url){return start("GET",url,new byte[0]);}public static int post(String url,byte[] body){return start("POST",url,body);}
 private static int start(String method,String url,byte[] body){if(body.length>32768)throw new IllegalArgumentException("HTTPS: тело больше 32 КиБ");return (int)Long.parseLong(ru.mcsa.bukkit.Host.text(40,method,url,hex(body)));}
 public static Response poll(int handle){String[] reply=ru.mcsa.bukkit.Host.call(41,Long.toString(handle&0xffffffffL));return reply.length==0?null:new Response(Integer.parseInt(reply[0]),unhex(reply[1]));}
 public static final class Response {private final int status;private final byte[] body;private Response(int status,byte[] body){this.status=status;this.body=body;}public int status(){return status;}public byte[] body(){return body.clone();}public String text(){return new String(body,java.nio.charset.StandardCharsets.UTF_8);}}
}
