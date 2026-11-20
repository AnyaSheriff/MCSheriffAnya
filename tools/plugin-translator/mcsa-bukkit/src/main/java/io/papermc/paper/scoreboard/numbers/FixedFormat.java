package io.papermc.paper.scoreboard.numbers;

import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.ComponentLike;

public interface FixedFormat extends NumberFormat, ComponentLike {
    Component component();
    default Component asComponent() { return component(); }
}
