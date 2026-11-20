package org.bukkit.event.player;

import java.util.Objects;
import org.bukkit.World;
import org.bukkit.entity.Player;

public class PlayerChangedWorldEvent extends PlayerEvent {
    private final World from;

    public PlayerChangedWorldEvent(Player player, World from) {
        super(Objects.requireNonNull(player));
        this.from = Objects.requireNonNull(from);
    }

    public World getFrom() { return from; }
}
