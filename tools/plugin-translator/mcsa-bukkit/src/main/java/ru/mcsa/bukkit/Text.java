package ru.mcsa.bukkit;
import java.util.*;import net.kyori.adventure.text.*;import net.kyori.adventure.text.format.*;
public final class Text implements TextComponent {
 private String cachedJson;private final String content;private final String color;private final Map<TextDecoration,Boolean> decorations;private final List<Component> children;
 public Text(String content){this(Objects.requireNonNull(content),null,new EnumMap<>(TextDecoration.class),new ArrayList<>());}
 private Text(String content,String color,Map<TextDecoration,Boolean> decorations,List<Component> children){this.content=Objects.requireNonNull(content);this.color=color;this.decorations=decorations;this.children=children;}
 public String content(){return content;}public TextComponent content(String value){return new Text(value,color,new EnumMap<>(decorations),new ArrayList<>(children));}
 public Text color(TextColor value){String hex=null;if(value!=null){int n=value.value();if(n<0||n>0xffffff)throw new IllegalArgumentException("Цвет: 0..ffffff");String digits=Integer.toHexString(n);hex="#"+"000000".substring(digits.length())+digits;}return new Text(content,hex,new EnumMap<>(decorations),new ArrayList<>(children));}
 public Text append(Component value){List<Component> c=new ArrayList<>(children);c.add(Objects.requireNonNull(value));return new Text(content,color,new EnumMap<>(decorations),c);}
 public Text decoration(TextDecoration d,boolean state){Map<TextDecoration,Boolean> m=new EnumMap<>(decorations);m.put(d,state);return new Text(content,color,m,new ArrayList<>(children));}
 // Один снимок стиля на сегмент вместо цепочки копий EnumMap для каждого цветового кода.
 public static Text legacy(String value,char marker){List<Component> parts=new ArrayList<>();StringBuilder text=new StringBuilder();Map<TextDecoration,Boolean> style=new EnumMap<>(TextDecoration.class);String color=null;for(int i=0;i<value.length();i++){char c=value.charAt(i);if(c==marker&&i+1<value.length()){char next=value.charAt(i+1);if(next>='A'&&next<='Z')next=(char)(next+32);int code="0123456789abcdefklmnor".indexOf(next);if(code>=0){parts.add(new Text(text.toString(),color,new EnumMap<>(style),new ArrayList<>()));text.setLength(0);i++;if(code<16||code==21){int n=code==21?0xffffff:NamedTextColor.COLORS[code].value();String digits=Integer.toHexString(n);color="#"+"000000".substring(digits.length())+digits;style.clear();for(TextDecoration d:TextDecoration.values())style.put(d,false);}else style.put(TextDecoration.values()[code-16],true);continue;}}text.append(c);}parts.add(new Text(text.toString(),color,new EnumMap<>(style),new ArrayList<>()));return new Text("",null,new EnumMap<>(TextDecoration.class),parts);}
 public static String quote(String text){StringBuilder out=new StringBuilder("\"");for(int i=0;i<text.length();i++){char c=text.charAt(i);if(c=='"'||c=='\\')out.append('\\').append(c);else if(c<32){out.append("\\u00").append("0123456789abcdef".charAt(c>>>4)).append("0123456789abcdef".charAt(c&15));}else out.append(c);}return out.append('"').toString();}
 public static String json(Component c){if(!(c instanceof Text))throw new IllegalArgumentException("Неизвестный компонент");return ((Text)c).json();}
 // Общие стили пустой обёртки переносятся в родителя только при явном стиле всех видимых детей.
 private boolean invisible(){return content.isEmpty()&&children.isEmpty();}
 private void writeJson(StringBuilder b,Map<TextDecoration,Boolean> inherited,String parentColor){
  Map<TextDecoration,Boolean> own=new EnumMap<>(decorations);
  if(content.isEmpty()&&!children.isEmpty())for(TextDecoration d:TextDecoration.values())if(!own.containsKey(d)){
   Boolean common=null;boolean found=false,shared=true;
   for(Component c:children){if(!(c instanceof Text))throw new IllegalArgumentException("Неизвестный компонент");Text child=(Text)c;if(child.invisible())continue;Boolean value=child.decorations.get(d);if(value==null||found&&!value.equals(common)){shared=false;break;}common=value;found=true;}
   if(found&&shared)own.put(d,common);
  }
  b.append("{\"text\":").append(quote(content));if(color!=null&&!color.equals(parentColor))b.append(",\"color\":").append(quote(color));
  for(Map.Entry<TextDecoration,Boolean> e:own.entrySet())if(!e.getValue().equals(inherited.get(e.getKey())))b.append(',').append(quote(Ascii.lower(e.getKey().name()))).append(':').append(e.getValue());
  Map<TextDecoration,Boolean> effective=new EnumMap<>(inherited);effective.putAll(own);boolean first=true;
  for(Component c:children){if(!(c instanceof Text))throw new IllegalArgumentException("Неизвестный компонент");Text child=(Text)c;if(child.invisible())continue;b.append(first?",\"extra\":[":",");first=false;child.writeJson(b,effective,color==null?parentColor:color);}
  if(!first)b.append(']');b.append('}');
 }
 public String json(){if(cachedJson==null){StringBuilder b=new StringBuilder();writeJson(b,new EnumMap<>(TextDecoration.class),null);cachedJson=b.toString();}return cachedJson;}

 public static Text fromJson(String json){return fromMap(Yaml.parse(json),0);}
 private static Text fromMap(Map<?,?> map,int depth){if(depth>32)throw new IllegalArgumentException("Слишком глубокий текст");String content=Objects.toString(map.get("text"),"");Map<TextDecoration,Boolean> decorations=new EnumMap<>(TextDecoration.class);for(TextDecoration d:TextDecoration.values())if(map.get(Ascii.lower(d.name())) instanceof Boolean)decorations.put(d,(Boolean)map.get(Ascii.lower(d.name())));List<Component> children=new ArrayList<>();if(map.get("extra") instanceof List)for(Object child:(List<?>)map.get("extra"))children.add(fromMap((Map<?,?>)child,depth+1));return new Text(content,(String)map.get("color"),decorations,children);}
 public String plain(){StringBuilder b=new StringBuilder(content);for(Component c:children)b.append(((Text)c).plain());return b.toString();}
 public boolean equals(Object value){if(!(value instanceof Text))return false;Text other=(Text)value;return content.equals(other.content)&&Objects.equals(color,other.color)&&decorations.equals(other.decorations)&&children.equals(other.children);}
 public int hashCode(){return Objects.hash(content,color,decorations,children);}
}
