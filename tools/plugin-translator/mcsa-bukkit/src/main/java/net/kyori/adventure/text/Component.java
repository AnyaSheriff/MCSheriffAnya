package net.kyori.adventure.text;
import net.kyori.adventure.text.format.*;
public interface Component {
 static TextComponent empty(){return text("");} static TextComponent text(String text){return new ru.mcsa.bukkit.Text(text);}
 static TextComponent text(String text,TextColor color){return new ru.mcsa.bukkit.Text(text).color(color);}
 Component append(Component child); Component color(TextColor color); Component decoration(TextDecoration decoration,boolean state);
}
