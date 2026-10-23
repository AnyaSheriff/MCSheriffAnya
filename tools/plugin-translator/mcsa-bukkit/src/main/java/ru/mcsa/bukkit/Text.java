package ru.mcsa.bukkit;
import java.util.*;import net.kyori.adventure.text.*;import net.kyori.adventure.text.format.*;
public final class Text implements TextComponent {
 private final String content;private final String color;private final Map<TextDecoration,Boolean> decorations;private final List<Component> children;
 public Text(String content){this(Objects.requireNonNull(content),null,new EnumMap<>(TextDecoration.class),new ArrayList<>());}
 private Text(String content,String color,Map<TextDecoration,Boolean> decorations,List<Component> children){this.content=Objects.requireNonNull(content);this.color=color;this.decorations=decorations;this.children=children;}
 public String content(){return content;}public TextComponent content(String value){return new Text(value,color,new EnumMap<>(decorations),new ArrayList<>(children));}
 public Text color(TextColor value){String hex=null;if(value!=null){int n=value.value();if(n<0||n>0xffffff)throw new IllegalArgumentException("Цвет: 0..ffffff");String digits=Integer.toHexString(n);hex="#"+"000000".substring(digits.length())+digits;}return new Text(content,hex,new EnumMap<>(decorations),new ArrayList<>(children));}
 public Text append(Component value){List<Component> c=new ArrayList<>(children);c.add(Objects.requireNonNull(value));return new Text(content,color,new EnumMap<>(decorations),c);}
 public Text decoration(TextDecoration d,boolean state){Map<TextDecoration,Boolean> m=new EnumMap<>(decorations);m.put(d,state);return new Text(content,color,m,new ArrayList<>(children));}
 public static String quote(String text){StringBuilder out=new StringBuilder("\"");for(int i=0;i<text.length();i++){char c=text.charAt(i);if(c=='"'||c=='\\')out.append('\\').append(c);else if(c<32){out.append("\\u00").append("0123456789abcdef".charAt(c>>>4)).append("0123456789abcdef".charAt(c&15));}else out.append(c);}return out.append('"').toString();}
 public static String json(Component c){if(!(c instanceof Text))throw new IllegalArgumentException("Неизвестный компонент");return ((Text)c).json();}
 public String json(){StringBuilder b=new StringBuilder("{\"text\":").append(quote(content));if(color!=null)b.append(",\"color\":").append(quote(color));for(Map.Entry<TextDecoration,Boolean> e:decorations.entrySet())b.append(',').append(quote(e.getKey().name().toLowerCase(Locale.ROOT))).append(':').append(e.getValue());if(!children.isEmpty()){b.append(",\"extra\":[");for(int i=0;i<children.size();i++){if(i>0)b.append(',');b.append(json(children.get(i)));}b.append(']');}return b.append('}').toString();}
 public String plain(){StringBuilder b=new StringBuilder(content);for(Component c:children)b.append(((Text)c).plain());return b.toString();}
 public boolean equals(Object value){if(!(value instanceof Text))return false;Text other=(Text)value;return content.equals(other.content)&&Objects.equals(color,other.color)&&decorations.equals(other.decorations)&&children.equals(other.children);}
 public int hashCode(){return Objects.hash(content,color,decorations,children);}
}
