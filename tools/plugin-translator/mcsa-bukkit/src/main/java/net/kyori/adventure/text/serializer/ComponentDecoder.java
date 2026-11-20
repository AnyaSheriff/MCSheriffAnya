package net.kyori.adventure.text.serializer;

import net.kyori.adventure.text.Component;

public interface ComponentDecoder<S, O extends Component> {
    O deserialize(S input);
}
