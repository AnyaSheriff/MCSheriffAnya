package net.kyori.adventure.text.format;
public final class NamedTextColor implements TextColor {
 private final int value;private final String name;private NamedTextColor(String name,int value){this.name=name;this.value=value;}public int value(){return value;}public String toString(){return name;}
public static final NamedTextColor BLACK=new NamedTextColor("black",0);
public static final NamedTextColor DARK_BLUE=new NamedTextColor("dark_blue",170);
public static final NamedTextColor DARK_GREEN=new NamedTextColor("dark_green",43520);
public static final NamedTextColor DARK_AQUA=new NamedTextColor("dark_aqua",43690);
public static final NamedTextColor DARK_RED=new NamedTextColor("dark_red",11141120);
public static final NamedTextColor DARK_PURPLE=new NamedTextColor("dark_purple",11141290);
public static final NamedTextColor GOLD=new NamedTextColor("gold",16755200);
public static final NamedTextColor GRAY=new NamedTextColor("gray",11184810);
public static final NamedTextColor DARK_GRAY=new NamedTextColor("dark_gray",5592405);
public static final NamedTextColor BLUE=new NamedTextColor("blue",5592575);
public static final NamedTextColor GREEN=new NamedTextColor("green",5635925);
public static final NamedTextColor AQUA=new NamedTextColor("aqua",5636095);
public static final NamedTextColor RED=new NamedTextColor("red",16733525);
public static final NamedTextColor LIGHT_PURPLE=new NamedTextColor("light_purple",16733695);
public static final NamedTextColor YELLOW=new NamedTextColor("yellow",16777045);
public static final NamedTextColor WHITE=new NamedTextColor("white",16777215);
public static final NamedTextColor[] COLORS={BLACK,DARK_BLUE,DARK_GREEN,DARK_AQUA,DARK_RED,DARK_PURPLE,GOLD,GRAY,DARK_GRAY,BLUE,GREEN,AQUA,RED,LIGHT_PURPLE,YELLOW,WHITE};
 public static final net.kyori.adventure.util.Index<String,NamedTextColor> NAMES;
 static {java.util.Map<String,NamedTextColor> values=new java.util.LinkedHashMap<>();for(NamedTextColor c:COLORS)values.put(c.name,c);NAMES=new net.kyori.adventure.util.Index<>(values);}
}
