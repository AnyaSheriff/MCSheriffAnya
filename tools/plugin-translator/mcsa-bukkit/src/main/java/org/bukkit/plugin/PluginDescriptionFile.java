package org.bukkit.plugin;

import java.util.Objects;

public final class PluginDescriptionFile {
    private final String name;
    private final String version;
    private final String main;

    public PluginDescriptionFile(String name, String version, String main) {
        this.name = Objects.requireNonNull(name);
        this.version = Objects.requireNonNull(version);
        this.main = Objects.requireNonNull(main);
    }

    public String getName() { return name; }
    public String getVersion() { return version; }
    public String getMain() { return main; }
    public String getFullName() { return name + " v" + version; }
}
