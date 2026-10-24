package fixtures;
import org.bukkit.command.*;
import org.bukkit.plugin.java.JavaPlugin;
public final class CommandPlugin extends JavaPlugin {
 @Override public boolean onCommand(CommandSender sender,Command command,String label,String[] args){
  if(args.length>0&&args[0].equals("persistent")&&sender instanceof org.bukkit.entity.Player){org.bukkit.entity.Player player=(org.bukkit.entity.Player)sender;Integer value=player.getPersistentDataContainer().get(new org.bukkit.NamespacedKey(this,"probe"),org.bukkit.persistence.PersistentDataType.INTEGER);if(value==null||value!=7)throw new IllegalStateException("PDC не пережил перезагрузку компонента");sender.sendMessage("Entity PDC survived reload");return true;}
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
   org.bukkit.inventory.ItemStack item=new org.bukkit.inventory.ItemStack(org.bukkit.Material.DIAMOND_SWORD);
   org.bukkit.inventory.meta.ItemMeta meta=item.getItemMeta();meta.displayName(net.kyori.adventure.text.Component.text("PDC Blade").color(net.kyori.adventure.text.format.NamedTextColor.RED));
   meta.lore(java.util.List.of(net.kyori.adventure.text.Component.text("Ability: blink")));meta.getPersistentDataContainer().set(key,org.bukkit.persistence.PersistentDataType.STRING,"blink");
   ((org.bukkit.inventory.meta.Damageable)meta).setDamage(3);((org.bukkit.inventory.meta.Damageable)meta).setMaxDamage(100);item.setItemMeta(meta);player.getInventory().setItem(8,item);
   org.bukkit.inventory.ItemStack read=player.getInventory().getItem(8);if(read==null||!"blink".equals(read.getPersistentDataContainer().get(key,org.bukkit.persistence.PersistentDataType.STRING)))throw new IllegalStateException("PDC не вернулся через хозяина");
   player.sendMessage("Item meta/PDC round trip");
   player.getPersistentDataContainer().set(key,org.bukkit.persistence.PersistentDataType.INTEGER,7);
   org.bukkit.attribute.AttributeInstance attack=player.getAttribute(org.bukkit.attribute.Attribute.ATTACK_DAMAGE);double base=attack.getBaseValue();attack.setBaseValue(7);attack.addTransientModifier(new org.bukkit.attribute.AttributeModifier(key,0.5,org.bukkit.attribute.AttributeModifier.Operation.ADD_SCALAR));
   if(attack.getValue()!=10.5)throw new IllegalStateException("Правки атрибутов не видны");attack.removeModifier(key);attack.setBaseValue(base);player.sendMessage("Attribute queue round trip");
   boolean denied=false;try{org.mcsa.api.Https.get("https://example.org/");}catch(IllegalStateException expected){denied=true;}if(!denied)throw new IllegalStateException("HTTPS без разрешения");player.sendMessage("HTTPS denied without approval");
   denied=false;try{new java.io.File(getDataFolder(),"../escape");}catch(SecurityException expected){denied=true;}if(!denied)throw new IllegalStateException("Выход из папки данных");player.sendMessage("Data folder confinement works");
   player.setHealth(18);player.addPotionEffect(new org.bukkit.potion.PotionEffect(org.bukkit.potion.PotionEffectType.NIGHT_VISION,100,0,false,false,true));
   return true;
  }
  sender.sendMessage("Команда из Java работает");return true;
 }
 @Override public java.util.List<String> onTabComplete(CommandSender sender,Command command,String label,String[] args){return java.util.Arrays.asList("java-tab-ok","java-tab-two");}
}
