package fixtures;
import org.bukkit.Bukkit;
import org.bukkit.plugin.java.JavaPlugin;
public final class SchedulerPlugin extends JavaPlugin implements org.bukkit.event.Listener {
 private final java.util.concurrent.ExecutorService writer=java.util.concurrent.Executors.newSingleThreadExecutor(r->{Thread thread=new Thread(r,getName()+"-writer");thread.setDaemon(true);return thread;});
 @Override public void onDisable(){writer.shutdown();try{if(!writer.awaitTermination(1,java.util.concurrent.TimeUnit.SECONDS))throw new IllegalStateException("queue pending");}catch(InterruptedException e){throw new IllegalStateException(e);}}
 @Override public void onEnable(){Bukkit.getPluginManager().registerEvents(this,this);saveDefaultConfig();long period=getConfig().getLong("period",20);Bukkit.getScheduler().runTaskTimer(this,()->Bukkit.broadcastMessage(getConfig().getString("message")),period,period);}
 @org.bukkit.event.EventHandler public void join(org.bukkit.event.player.PlayerJoinEvent event){writer.execute(()->Bukkit.broadcastMessage("Ordered executor first"));writer.execute(()->{Bukkit.broadcastMessage("Ordered executor second");try{java.nio.file.Path path=new java.io.File(getDataFolder(),"queue.yml").toPath();java.nio.file.Files.writeString(path,"order: [first, second]",java.nio.charset.StandardCharsets.UTF_8);Bukkit.broadcastMessage("Ordered executor saved");}catch(java.io.IOException e){throw new IllegalStateException(e);}});}
}
