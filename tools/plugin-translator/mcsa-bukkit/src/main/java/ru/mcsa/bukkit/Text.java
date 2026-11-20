package ru.mcsa.bukkit;
import java.util.*;import net.kyori.adventure.text.*;import net.kyori.adventure.text.format.*;
public final class Text implements TextComponent {
 private String cachedJson;private final String content;private final String color;private final Map<TextDecoration,Boolean> decorations;private final List<Component> children;
 public Text(String content){this(Objects.requireNonNull(content),null,new EnumMap<>(TextDecoration.class),new ArrayList<>());}
 private Text(String content,String color,Map<TextDecoration,Boolean> decorations,List<Component> children){this.content=Objects.requireNonNull(content);this.color=color;this.decorations=decorations;this.children=children;}
 public String content(){return content;}public TextComponent content(String value){return new Text(value,color,new EnumMap<>(decorations),new ArrayList<>(children));}
 public Text color(TextColor value){String hex=null;if(value!=null){int n=value.value();if(n<0||n>0xffffff)throw new IllegalArgumentException("Цвет: 0..ffffff");hex=hexColor(n);}return new Text(content,hex,new EnumMap<>(decorations),new ArrayList<>(children));}
 public Text append(Component value){List<Component> c=new ArrayList<>(children);c.add(Objects.requireNonNull(value));return new Text(content,color,new EnumMap<>(decorations),c);}
 public Text decoration(TextDecoration d,boolean state){Map<TextDecoration,Boolean> m=new EnumMap<>(decorations);m.put(d,state);return new Text(content,color,m,new ArrayList<>(children));}
 private static final String LEGACY_COLORS = "0123456789abcdef";
 private static final String LEGACY_DECORATIONS = "klmno";

 // Один снимок стиля на сегмент вместо цепочки копий при каждом коде.
 public static Text legacy(String value, char marker) {
  List<Component> parts = new ArrayList<>();
  StringBuilder text = new StringBuilder();
  Map<TextDecoration,Boolean> style = new EnumMap<>(TextDecoration.class);
  String color = null;
  for (int i = 0; i < value.length(); i++) {
   char current = value.charAt(i);
   if (current == marker && i + 1 < value.length()) {
    char code = Character.toLowerCase(value.charAt(i + 1));
    int named = LEGACY_COLORS.indexOf(code);
    int decoration = LEGACY_DECORATIONS.indexOf(code);
    String rgb = code == '#' || code == 'x' ? legacyRgb(value, i + 2, marker, code == 'x') : null;
    if (named >= 0 || decoration >= 0 || code == 'r' || rgb != null) {
     if (text.length() > 0) {
      parts.add(legacyPart(text, color, style));
      text.setLength(0);
     }
     i += rgb == null ? 1 : code == 'x' ? 13 : 7;
     if (decoration >= 0) {
      style.put(TextDecoration.values()[decoration], true);
     } else {
      color = rgb == null ? hexColor(named >= 0 ? NamedTextColor.COLORS[named].value() : 0xffffff) : rgb;
      for (TextDecoration d : TextDecoration.values()) style.put(d, false);
     }
     continue;
    }
   }
   text.append(current);
  }
  if (text.length() > 0) parts.add(legacyPart(text, color, style));
  return new Text("", null, new EnumMap<>(TextDecoration.class), parts);
 }
 private static Text legacyPart(StringBuilder text, String color, Map<TextDecoration,Boolean> style) {
  return new Text(text.toString(), color, new EnumMap<>(style), new ArrayList<>());
 }
 private static String hexColor(int color) {
  String digits = Integer.toHexString(color);
  return "#" + "000000".substring(digits.length()) + digits;
 }
 private static String legacyRgb(String value, int pos, char marker, boolean repeated) {
  if (value.length() - pos < (repeated ? 12 : 6)) return null;
  StringBuilder out = new StringBuilder("#");
  for (int i = 0; i < 6; i++) {
   if (repeated && value.charAt(pos++) != marker) return null;
   char digit = Character.toLowerCase(value.charAt(pos++));
   if (LEGACY_COLORS.indexOf(digit) < 0) return null;
   out.append(digit);
  }
  return out.toString();
 }

 public String toLegacy(char marker, boolean hex, boolean repeated) {
  LegacyOutput out = new LegacyOutput(marker, hex, repeated);
  appendLegacy(out, null, 0);
  return out.text.toString();
 }
 private void appendLegacy(LegacyOutput out, String inheritedColor, int inheritedStyle) {
  int style = inheritedStyle;
  for (Map.Entry<TextDecoration,Boolean> entry : decorations.entrySet()) {
   int bit = 1 << entry.getKey().ordinal();
   if (entry.getValue()) style |= bit;
   else style &= ~bit;
  }
  String effectiveColor = color == null ? inheritedColor : color;
  if (!content.isEmpty()) out.append(content, effectiveColor, style);
  for (Component child : children) {
   if (!(child instanceof Text)) throw new IllegalArgumentException("Неизвестный компонент");
   ((Text)child).appendLegacy(out, effectiveColor, style);
  }
 }
 private static final class LegacyOutput {
  final StringBuilder text = new StringBuilder();
  final char marker;
  final boolean hex, repeated;
  String lastColor;
  int lastStyle;

  LegacyOutput(char marker, boolean hex, boolean repeated) {
   this.marker = marker;
   this.hex = hex;
   this.repeated = repeated;
  }
  void append(String content, String color, int style) {
   String code = colorCode(color, hex);
   if (!Objects.equals(code, lastColor) || (lastStyle & ~style) != 0) {
    if (code == null) text.append(marker).append('r');
    else if (code.length() == 1 || !repeated) text.append(marker).append(code);
    else {
     text.append(marker).append('x');
     for (int i = 1; i < code.length(); i++) text.append(marker).append(code.charAt(i));
    }
    lastColor = code;
    lastStyle = 0;
   }
   for (int i = 0; i < LEGACY_DECORATIONS.length(); i++) {
    if (((style & ~lastStyle) & (1 << i)) != 0) text.append(marker).append(LEGACY_DECORATIONS.charAt(i));
   }
   lastStyle = style;
   text.append(content);
  }
 }
 private static String colorCode(String color, boolean hex) {
  if (color == null || color.equals("reset")) return null;
  int rgb = color.startsWith("#") ? Integer.parseInt(color.substring(1), 16) : NamedTextColor.NAMES.value(color).value();
  for (int i = 0; i < NamedTextColor.COLORS.length; i++) {
   if (NamedTextColor.COLORS[i].value() == rgb) return String.valueOf(LEGACY_COLORS.charAt(i));
  }
  if (hex) return hexColor(rgb);
  int nearest = 0;
  double distance = Double.POSITIVE_INFINITY;
  double[] original = hsv(rgb);
  for (int i = 0; i < NamedTextColor.COLORS.length; i++) {
   double[] candidate = hsv(NamedTextColor.COLORS[i].value());
   double hue = Math.abs(original[0] - candidate[0]);
   hue = 3 * Math.min(hue, 1 - hue);
   double saturation = original[1] - candidate[1];
   double brightness = original[2] - candidate[2];
   double next = hue * hue + saturation * saturation + brightness * brightness;
   if (next < distance) {
    distance = next;
    nearest = i;
   }
  }
  return String.valueOf(LEGACY_COLORS.charAt(nearest));
 }
 private static double[] hsv(int rgb) {
  int red = (rgb >>> 16) & 255, green = (rgb >>> 8) & 255, blue = rgb & 255;
  int max = Math.max(red, Math.max(green, blue));
  int range = max - Math.min(red, Math.min(green, blue));
  if (range == 0) return new double[]{0, 0, max / 255.0};
  double hue;
  if (max == red) hue = (green - blue) / (double)range;
  else if (max == green) hue = 2 + (blue - red) / (double)range;
  else hue = 4 + (red - green) / (double)range;
  return new double[]{(hue < 0 ? hue + 6 : hue) / 6, range / (double)max, max / 255.0};
 }
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

 public static Text fromJson(String json) {
  return fromValue(Yaml.flowValue(json), 0);
 }
 private static Text fromValue(Object value, int depth) {
  if (depth > 32) throw new IllegalArgumentException("Слишком глубокий текст");
  if (value instanceof String) return new Text((String)value);
  if (value instanceof List) {
   List<?> parts = (List<?>)value;
   if (parts.isEmpty()) return new Text("");
   Text first = fromValue(parts.get(0), depth + 1);
   List<Component> children = new ArrayList<>(first.children);
   for (int i = 1; i < parts.size(); i++) children.add(fromValue(parts.get(i), depth + 1));
   return new Text(first.content, first.color, first.decorations, children);
  }
  if (!(value instanceof Map)) throw new IllegalArgumentException("Неизвестный компонент");
  Map<?,?> map = (Map<?,?>)value;
  String content = Objects.toString(map.get("text"), Objects.toString(map.get("fallback"),
      Objects.toString(map.get("translate"), Objects.toString(map.get("keybind"), ""))));
  Map<TextDecoration,Boolean> decorations = new EnumMap<>(TextDecoration.class);
  for (TextDecoration decoration : TextDecoration.values()) {
   Object flag = map.get(Ascii.lower(decoration.name()));
   if (flag instanceof Boolean) decorations.put(decoration, (Boolean)flag);
  }
  List<Component> children = new ArrayList<>();
  if (map.get("extra") instanceof List) {
   for (Object child : (List<?>)map.get("extra")) children.add(fromValue(child, depth + 1));
  }
  return new Text(content, (String)map.get("color"), decorations, children);
 }
 public String plain(){StringBuilder b=new StringBuilder(content);for(Component c:children)b.append(((Text)c).plain());return b.toString();}
 public boolean equals(Object value){if(!(value instanceof Text))return false;Text other=(Text)value;return content.equals(other.content)&&Objects.equals(color,other.color)&&decorations.equals(other.decorations)&&children.equals(other.children);}
 public int hashCode(){return Objects.hash(content,color,decorations,children);}
}
