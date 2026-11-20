package net.kyori.adventure.text.minimessage;

import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.serializer.ComponentDecoder;
import ru.mcsa.bukkit.MiniText;

public interface MiniMessage extends ComponentDecoder<String, Component> {
    static MiniMessage miniMessage() { return MiniText.INSTANCE; }
}
