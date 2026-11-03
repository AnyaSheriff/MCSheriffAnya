package net.kyori.adventure.text;
import net.kyori.adventure.text.format.*;
public interface TextComponent extends Component {
 String content(); TextComponent content(String content);
 TextComponent append(Component child); TextComponent color(TextColor color); TextComponent decoration(TextDecoration decoration,boolean state);
}
