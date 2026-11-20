package org.bukkit.scoreboard;

import ru.mcsa.bukkit.Ascii;

public enum DisplaySlot {
    PLAYER_LIST("list"), SIDEBAR("sidebar"), BELOW_NAME("below_name"),
    SIDEBAR_TEAM_BLACK, SIDEBAR_TEAM_DARK_BLUE, SIDEBAR_TEAM_DARK_GREEN, SIDEBAR_TEAM_DARK_AQUA,
    SIDEBAR_TEAM_DARK_RED, SIDEBAR_TEAM_DARK_PURPLE, SIDEBAR_TEAM_GOLD, SIDEBAR_TEAM_GRAY,
    SIDEBAR_TEAM_DARK_GRAY, SIDEBAR_TEAM_BLUE, SIDEBAR_TEAM_GREEN, SIDEBAR_TEAM_AQUA,
    SIDEBAR_TEAM_RED, SIDEBAR_TEAM_LIGHT_PURPLE, SIDEBAR_TEAM_YELLOW, SIDEBAR_TEAM_WHITE;

    private final String protocolName;

    DisplaySlot() { this.protocolName = "sidebar.team." + Ascii.lower(name().substring("SIDEBAR_TEAM_".length())); }
    DisplaySlot(String protocolName) { this.protocolName = protocolName; }
    public String protocolName() { return protocolName; }
}
