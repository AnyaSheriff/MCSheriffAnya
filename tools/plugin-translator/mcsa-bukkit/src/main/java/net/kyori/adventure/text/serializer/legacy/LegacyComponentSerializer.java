package net.kyori.adventure.text.serializer.legacy;

import java.util.LinkedHashMap;
import java.util.Map;
import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.TextComponent;
import ru.mcsa.bukkit.Text;

public final class LegacyComponentSerializer {
    private final Map<String, TextComponent> cache = new LinkedHashMap<>();
    private final char marker;
    private final boolean hexColors;
    private final boolean repeatedHex;

    private LegacyComponentSerializer(char marker, boolean hexColors, boolean repeatedHex) {
        this.marker = marker;
        this.hexColors = hexColors;
        this.repeatedHex = repeatedHex;
    }

    public static LegacyComponentSerializer legacyAmpersand() {
        return builder().character('&').build();
    }

    public static LegacyComponentSerializer legacySection() {
        return builder().build();
    }

    public static Builder builder() {
        return new Builder();
    }

    public TextComponent deserialize(String value) {
        TextComponent found = cache.get(value);
        if (found != null) return found;
        TextComponent result = Text.legacy(value, marker);
        if (cache.size() >= 256) cache.remove(cache.keySet().iterator().next());
        cache.put(value, result);
        return result;
    }

    public String serialize(Component value) {
        if (!(value instanceof Text)) throw new IllegalArgumentException("Неизвестный компонент");
        return ((Text)value).toLegacy(marker, hexColors, repeatedHex);
    }

    public static final class Builder {
        private char marker = '§';
        private boolean hexColors;
        private boolean repeatedHex;

        private Builder() {}

        public Builder character(char marker) {
            this.marker = marker;
            return this;
        }

        public Builder hexColors() {
            hexColors = true;
            return this;
        }

        public Builder useUnusualXRepeatedCharacterHexFormat() {
            repeatedHex = true;
            return this;
        }

        public LegacyComponentSerializer build() {
            return new LegacyComponentSerializer(marker, hexColors, repeatedHex);
        }
    }
}
