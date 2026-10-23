package ru.mcsa.translator;
import org.bukkit.NamespacedKey;
import net.kyori.adventure.text.*;
import net.kyori.adventure.text.format.*;
import net.kyori.adventure.text.serializer.legacy.LegacyComponentSerializer;
import ru.mcsa.bukkit.Text;
public final class SdkContracts {
 private static void check(boolean value){if(!value)throw new AssertionError();}
 public static void main(String[] args){
  check(NamespacedKey.minecraft("a/b").toString().equals("minecraft:a/b"));
  check(NamespacedKey.fromString("STONE")==null);
  check(NamespacedKey.fromString("..:y")!=null);
  check(NamespacedKey.fromString("x:y:z")==null);
  check(NamespacedKey.fromString(":stone").equals(NamespacedKey.minecraft("stone")));
  check(NamespacedKey.minecraft("stone").hashCode()==NamespacedKey.fromString("stone").hashCode());
  TextComponent text=Component.text("A\n\"\\");
  TextComponent changed=text.append(Component.text("B")).decoration(TextDecoration.ITALIC,false);
  check(((Text)text).plain().equals("A\n\"\\"));
  check(((Text)changed).plain().equals("A\n\"\\B"));
  check(Text.json(text).equals("{\"text\":\"A\\u000a\\\"\\\\\"}"));
  check(Text.json(changed).contains("\"italic\":false"));
  TextComponent legacy=LegacyComponentSerializer.legacyAmpersand().deserialize("&aGreen&lBold&rWhite");
  check(((Text)legacy).plain().equals("GreenBoldWhite"));
  check(Text.json(legacy).contains("#55ff55"));
  check(Text.json(legacy).contains("\"bold\":true"));
  check(((Text)LegacyComponentSerializer.legacyAmpersand().deserialize("A&zB&")).plain().equals("A&zB&"));
  check(NamedTextColor.NAMES.value("white")==NamedTextColor.WHITE);
  check(Component.text("same").append(Component.text("child")).equals(Component.text("same").append(Component.text("child"))));
  System.out.println("PASS: 16 контрактов ключей и неизменяемых текстовых компонентов");
 }
}
