package fixtures;
public final class Failed extends org.bukkit.plugin.java.JavaPlugin {
 @Override public void onEnable(){throw new IllegalStateException("own fixture deliberate failed enable");}
}
