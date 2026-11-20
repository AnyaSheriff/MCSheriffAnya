package fixtures;
import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.format.NamedTextColor;
import net.kyori.adventure.text.format.TextDecoration;
import net.kyori.adventure.text.serializer.legacy.LegacyComponentSerializer;
import org.bukkit.Bukkit;
import org.bukkit.boss.*;
import org.bukkit.command.*;
import org.bukkit.entity.Player;
import org.bukkit.plugin.java.JavaPlugin;

// Собственная проверка API: ни Bukkit/Paper JAR, ни сторонние плагины не используются.
public final class BossBarPlugin extends JavaPlugin {
 private BossBar bar,second;
 private final java.util.List<BossBar> capacityBars=new java.util.ArrayList<>();
 private static void check(boolean value){if(!value)throw new IllegalStateException("Проверка BossBar не прошла");}
 @Override public void onEnable(){
  bar=Bukkit.createBossBar("\u00a7aПервая",BarColor.BLUE,BarStyle.SEGMENTED_10);
  second=getServer().createBossBar(null,BarColor.RED,BarStyle.SOLID);
  check(bar.isVisible()&&bar.getProgress()==1&&bar.getPlayers().isEmpty());
  check(second.getTitle().isEmpty());
 }
 // Проверка отказа выполняется отдельным методом: после catch вызывающий
 // по-прежнему проверяет список зрителей у того же объекта BossBar.
 private static void expectAddRejected(BossBar rejected, Player player){
  try{
   rejected.addPlayer(player);
  }catch(IllegalStateException expected){
   check(expected.getMessage().contains("отклонена хозяином"));
   return;
  }
  throw new IllegalStateException("Лимит хозяина не сработал");
 }
 private static void expectShowRejected(BossBar target){
  try{
   target.setVisible(true);
  }catch(IllegalStateException expected){
   check(expected.getMessage().contains("отклонена хозяином")&&expected.getSuppressed().length==0);
   return;
  }
  throw new IllegalStateException("Лимит хозяина не сработал");
 }
 private static void checkSuppressedExceptions(){
  Throwable failure=new IllegalStateException("Основная ошибка");
  check(failure.getSuppressed().length==0);
  Throwable first=new IllegalArgumentException("Первая");
  Throwable second=new IllegalArgumentException("Вторая");
  failure.addSuppressed(first);
  failure.addSuppressed(second);
  Throwable[] snapshot=failure.getSuppressed();
  check(snapshot.length==2&&snapshot[0]==first&&snapshot[1]==second);
  snapshot[0]=null;
  check(failure.getSuppressed()[0]==first);
 }
 private static void setTabPresentation(Player player) {
  LegacyComponentSerializer serializer = LegacyComponentSerializer.builder().character('&')
      .hexColors().useUnusualXRepeatedCharacterHexFormat().build();
  Component text = Component.text("Name", NamedTextColor.AQUA)
      .decoration(TextDecoration.BOLD, true)
      .append(Component.newline())
      .append(Component.text("Line").decoration(TextDecoration.BOLD, false));
  check(serializer.serialize(text).equals("&b&lName\n&bLine"));
  String legacy = "&x&1&2&3&4&a&b&lName\n&x&1&2&3&4&a&bLine";
  check(serializer.serialize(serializer.deserialize(legacy)).equals(legacy));
  player.setDisplayName(legacy.replace('&', '§'));
  check(player.getDisplayName().equals(legacy.replace('&', '§')));
  check(Bukkit.getPlayer(player.getUniqueId()).getDisplayName().equals(player.getDisplayName()));
  player.playerListName(Component.text("TabAlias"));
  player.sendPlayerListHeaderAndFooter(Component.text("TabHead").append(Component.newline()).append(Component.text("LineTwo")), Component.text("TabFoot"));
 }
 @Override public boolean onCommand(CommandSender sender,Command command,String label,String[] args){
  if(!(sender instanceof Player)||args.length!=1)return false;
  Player player=(Player)sender;
  if(args[0].equals("info")){
   sender.sendMessage("PlayerInfo "+player.getPing()+" "+Bukkit.getMaxPlayers()+" "+getServer().getMaxPlayers());
   return true;
  }
  if (args[0].equals("tab")) {
   check(player.getDisplayName().equals(player.getName()));
   setTabPresentation(player);
   sender.sendMessage("Tab updated");
   return true;
  }
  if (args[0].equals("tab-reset")) {
   player.playerListName(null);
   player.setDisplayName(null);
   check(player.getDisplayName().equals(player.getName()));
   sender.sendMessage("Tab reset");
   return true;
  }
  switch(args[0]){
   case "add":
    bar.addPlayer(player);bar.addPlayer(Bukkit.getPlayer(player.getUniqueId()));
    check(bar.getPlayers().contains(player));
    try{bar.getPlayers().clear();throw new IllegalStateException("Список зрителей изменяем");}catch(UnsupportedOperationException expected){}
    break;
   case "update":bar.setProgress(0.25);bar.setTitle("\u00a7cНовая");bar.setColor(BarColor.GREEN);bar.setStyle(BarStyle.SEGMENTED_6);check(bar.getProgress()==0.25&&bar.getTitle().equals("\u00a7cНовая")&&bar.getColor()==BarColor.GREEN&&bar.getStyle()==BarStyle.SEGMENTED_6);break;
   case "hide":bar.setVisible(false);bar.setVisible(false);check(!bar.isVisible());break;
   case "hidden":check(!bar.isVisible());bar.setProgress(0.75);bar.setTitle("Скрытая");bar.addPlayer(player);break;
   case "show":bar.setVisible(true);bar.setVisible(true);check(bar.isVisible());break;
   case "remove":bar.removePlayer(player);bar.removePlayer(player);check(!bar.getPlayers().contains(player));break;
   case "all":bar.removeAll();bar.removeAll();check(bar.getPlayers().isEmpty());break;
   case "second":second.addPlayer(player);break;
   case "bounds":bar.setProgress(0);check(bar.getProgress()==0);bar.setProgress(1);check(bar.getProgress()==1);break;
   case "fill-hidden-viewer":
    check(!bar.isVisible()&&bar.getPlayers().size()==2&&capacityBars.isEmpty());
    for(int i=0;i<32;i++){BossBar extra=Bukkit.createBossBar("Занято "+i,BarColor.RED,BarStyle.SOLID);extra.addPlayer(player);capacityBars.add(extra);}
    break;
   case "show-rejected":
    check(!bar.isVisible()&&bar.getPlayers().size()==2&&capacityBars.size()==32);
    expectShowRejected(bar);
    check(!bar.isVisible()&&bar.getPlayers().size()==2);
    break;
   case "free-hidden-viewer":
    check(!bar.isVisible()&&capacityBars.size()==32);for(BossBar extra:capacityBars)extra.removeAll();capacityBars.clear();break;
   case "limit":
    check(bar.getPlayers().isEmpty()&&second.getPlayers().isEmpty());
    java.util.List<BossBar> bars=new java.util.ArrayList<>();
    for(int i=0;i<32;i++){BossBar extra=Bukkit.createBossBar("Лимит "+i,BarColor.RED,BarStyle.SOLID);extra.addPlayer(player);bars.add(extra);}
    BossBar rejected=Bukkit.createBossBar("Сверх лимита",BarColor.RED,BarStyle.SOLID);
    expectAddRejected(rejected,player);
    check(rejected.getPlayers().isEmpty());
    for(BossBar extra:bars)extra.removeAll();
    break;
   case "invalid":
    checkSuppressedExceptions();
    for(double value:new double[]{Double.NaN,Double.POSITIVE_INFINITY,Double.NEGATIVE_INFINITY,-0.01,1.01}){
     double before=bar.getProgress();try{bar.setProgress(value);throw new IllegalStateException("Неверное заполнение принято");}catch(IllegalArgumentException expected){}check(bar.getProgress()==before);
    }
    try{bar.setColor(null);throw new IllegalStateException("null цвет принят");}catch(NullPointerException expected){}
    try{bar.setStyle(null);throw new IllegalStateException("null деления приняты");}catch(NullPointerException expected){}
    try{bar.addPlayer(null);throw new IllegalStateException("null игрок принят");}catch(NullPointerException expected){}
    try{bar.removePlayer(null);throw new IllegalStateException("null игрок принят");}catch(NullPointerException expected){}
    try{Bukkit.createBossBar("",BarColor.RED,BarStyle.SOLID,(BarFlag[])null);throw new IllegalStateException("null массив флагов принят");}catch(NullPointerException expected){}
    try{Bukkit.createBossBar("",BarColor.RED,BarStyle.SOLID,new BarFlag[]{null});throw new IllegalStateException("Неподдержанный флаг принят");}catch(UnsupportedOperationException expected){}
    StringBuilder longTitle=new StringBuilder();for(int i=0;i<17000;i++)longTitle.append('x');
    String beforeTitle=bar.getTitle();try{bar.setTitle(longTitle.toString());throw new IllegalStateException("Слишком длинный заголовок принят");}catch(IllegalArgumentException expected){}check(bar.getTitle().equals(beforeTitle));
    break;
   default:return false;
  }
  sender.sendMessage("BossBar "+args[0]+" "+bar.getPlayers().size());return true;
 }
}
