package fixtures;
import org.bukkit.command.*;
import org.bukkit.plugin.java.JavaPlugin;
public final class CommandPlugin extends JavaPlugin {
 @Override public boolean onCommand(CommandSender sender,Command command,String label,String[] args){
  if(args.length>0&&args[0].equals("zero")&&sender instanceof org.bukkit.entity.Player){((org.bukkit.entity.Player)sender).setHealth(0);return true;}
  if(args.length>0&&args[0].equals("api")&&sender instanceof org.bukkit.entity.Player){
   org.bukkit.entity.Player player=(org.bukkit.entity.Player)sender;
   java.util.concurrent.ConcurrentHashMap<String,Integer> values=new java.util.concurrent.ConcurrentHashMap<>();values.put("value",3);
   java.util.concurrent.atomic.AtomicInteger count=new java.util.concurrent.atomic.AtomicInteger(values.get("value"));
   int roll=java.util.concurrent.ThreadLocalRandom.current().nextInt(1,4);if(roll<1||roll>=4||java.util.concurrent.TimeUnit.SECONDS.toMillis(1)!=1000)throw new IllegalStateException("Случайное число или единица времени");
   org.bukkit.NamespacedKey key=new org.bukkit.NamespacedKey(this,"probe");
   if(!key.toString().equals("java-command:probe")||count.incrementAndGet()!=4)throw new IllegalStateException("Карта, атомарное число или ключ");
   net.kyori.adventure.text.Component text=net.kyori.adventure.text.serializer.legacy.LegacyComponentSerializer.legacyAmpersand().deserialize("&aКомпонент из Java работает").decoration(net.kyori.adventure.text.format.TextDecoration.ITALIC,false);
   player.sendMessage(text);player.sendActionBar(net.kyori.adventure.text.Component.text("Java action bar"));
   player.setHealth(18);player.addPotionEffect(new org.bukkit.potion.PotionEffect(org.bukkit.potion.PotionEffectType.NIGHT_VISION,100,0,false,false,true));
   return true;
  }
  sender.sendMessage("Команда из Java работает");return true;
 }
 @Override public java.util.List<String> onTabComplete(CommandSender sender,Command command,String label,String[] args){return java.util.Arrays.asList("java-tab-ok","java-tab-two");}
}
