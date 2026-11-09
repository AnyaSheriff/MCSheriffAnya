package fixtures;
import org.bukkit.command.*;
import org.bukkit.enchantments.Enchantment;
import org.bukkit.entity.Player;
import org.bukkit.inventory.ItemStack;
import org.bukkit.inventory.meta.ItemMeta;
import org.bukkit.plugin.java.JavaPlugin;
public final class EnchantmentPlugin extends JavaPlugin {
 @Override public boolean onCommand(CommandSender sender,Command command,String label,String[] args){
  if(!(sender instanceof Player)||args.length!=1)return false;
  ItemStack item=((Player)sender).getInventory().getItem(0);
  if(item==null)throw new IllegalStateException("Нет настоящего предмета в первой ячейке");
  if(args[0].equals("direct")){
   int removed=item.removeEnchantment(Enchantment.SHARPNESS),repeated=item.removeEnchantment(Enchantment.SHARPNESS);
   ItemMeta meta=item.getItemMeta();
   if(removed!=3||repeated!=0||!meta.hasDisplayName()||!meta.hasEnchants()||!meta.hasEnchant(Enchantment.UNBREAKING)||meta.getEnchantLevel(Enchantment.UNBREAKING)!=2)throw new IllegalStateException("Снятие ItemStack потеряло уровень или соседние метаданные");
   sender.sendMessage("Removed stack 3; repeat 0");return true;
  }
  if(args[0].equals("meta")){
   ItemMeta meta=item.getItemMeta();boolean removed=meta.removeEnchant(Enchantment.UNBREAKING),repeated=meta.removeEnchant(Enchantment.UNBREAKING);
   if(!removed||repeated||meta.hasEnchants()||meta.hasEnchant(Enchantment.UNBREAKING)||meta.getEnchantLevel(Enchantment.UNBREAKING)!=0||!meta.hasDisplayName()||item.getEnchantmentLevel(Enchantment.UNBREAKING)!=2)throw new IllegalStateException("Снятие последнего зачарования или отдельная ItemMeta нарушены");
   if(!item.setItemMeta(meta))throw new IllegalStateException("Запись ItemMeta отклонена");
   sender.sendMessage("Removed meta true; repeat false");return true;
  }
  if(args[0].equals("absent")){
   if(item.removeEnchantment(Enchantment.SHARPNESS)!=0||item.getItemMeta().removeEnchant(Enchantment.UNBREAKING))throw new IllegalStateException("Повторное снятие изменило пустую карту");
   sender.sendMessage("Absent enchantments unchanged");return true;
  }
  return false;
 }
}
