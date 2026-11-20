package io.papermc.paper.scoreboard.numbers;

import java.util.Objects;
import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.ComponentLike;

public interface NumberFormat {
    static FixedFormat fixed(ComponentLike component) {
        Component text = Objects.requireNonNull(component.asComponent());
        return () -> text;
    }
}
