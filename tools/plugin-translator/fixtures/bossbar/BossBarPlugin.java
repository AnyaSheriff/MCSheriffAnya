package fixtures;
import io.papermc.paper.scoreboard.numbers.NumberFormat;
import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.format.NamedTextColor;
import net.kyori.adventure.text.format.TextDecoration;
import net.kyori.adventure.text.minimessage.MiniMessage;
import net.kyori.adventure.text.serializer.legacy.LegacyComponentSerializer;
import org.bukkit.Bukkit;
import org.bukkit.Location;
import org.bukkit.Material;
import org.bukkit.Particle;
import org.bukkit.Sound;
import org.bukkit.boss.*;
import org.bukkit.command.*;
import org.bukkit.entity.Player;
import org.bukkit.event.EventHandler;
import org.bukkit.event.Listener;
import org.bukkit.event.player.PlayerChangedWorldEvent;
import org.bukkit.inventory.ItemStack;
import org.bukkit.inventory.PlayerInventory;
import org.bukkit.plugin.java.JavaPlugin;
import org.bukkit.potion.PotionEffectType;
import org.bukkit.scoreboard.DisplaySlot;
import org.bukkit.scoreboard.Objective;
import org.bukkit.scoreboard.Score;
import org.bukkit.scoreboard.Scoreboard;
import org.bukkit.scoreboard.Team;

// Собственная проверка API: ни Bukkit/Paper JAR, ни сторонние плагины не используются.
public final class BossBarPlugin extends JavaPlugin implements Listener {
 private BossBar bar,second;
 private final java.util.List<BossBar> capacityBars=new java.util.ArrayList<>();
 private static void check(boolean value){if(!value)throw new IllegalStateException("Проверка BossBar не прошла");}
 @Override public void onEnable(){
  check(getDescription().getName().equals("java-bossbar") && getDescription().getVersion().equals("1.0")
      && getDescription().getMain().equals("fixtures.BossBarPlugin"));
  Bukkit.getPluginManager().registerEvents(this,this);
  bar=Bukkit.createBossBar("\u00a7aПервая",BarColor.BLUE,BarStyle.SEGMENTED_10);
  second=getServer().createBossBar(null,BarColor.RED,BarStyle.SOLID);
  check(bar.isVisible()&&bar.getProgress()==1&&bar.getPlayers().isEmpty());
  check(second.getTitle().isEmpty());
 }
 @EventHandler public void onWorldChanged(PlayerChangedWorldEvent event) {
  Player player = event.getPlayer();
  player.sendMessage("ChangedWorld " + event.getFrom().getName() + " " + player.getWorld().getName());
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
 private static void checkRacePresentation(Player player) {
  check(player.hasPotionEffect(PotionEffectType.SPEED) && !player.hasPotionEffect(PotionEffectType.POISON));
  PlayerInventory inventory = player.getInventory();
  inventory.setItemInMainHand(new ItemStack(Material.IRON_SWORD));
  inventory.setBoots(new ItemStack(Material.IRON_BOOTS));
  inventory.setLeggings(new ItemStack(Material.IRON_LEGGINGS));
  inventory.setChestplate(new ItemStack(Material.IRON_CHESTPLATE));
  inventory.setHelmet(new ItemStack(Material.IRON_HELMET));
  check(inventory.getItemInMainHand().getType() == Material.IRON_SWORD);
  check(inventory.getArmorContents()[1].getType() == Material.IRON_LEGGINGS);
  inventory.setLeggings(null);
  check(inventory.getArmorContents()[1] == null);
  Location at = player.getLocation();
  player.getWorld().playSound(at, Sound.ENTITY_WARDEN_SONIC_CHARGE, 0.6f, 1.2f);
  player.getWorld().spawnParticle(Particle.TOTEM_OF_UNDYING, at, 3, 0.25, 0.5, 1);
  player.sendActionBar("§aRace ready");
 }
 private static void setMainBoard(Player player) {
  Scoreboard main = Bukkit.getScoreboardManager().getMainScoreboard();
  check(player.getScoreboard() == main);
  Objective objective = main.registerNewObjective("compat", "dummy", Component.text("MainTitle"));
  objective.setDisplaySlot(DisplaySlot.BELOW_NAME);
  Score score = objective.getScore(player.getName());
  check(!score.isScoreSet() && score.getScore() == 0 && main.getEntries().isEmpty());
  score.numberFormat(NumberFormat.fixed(Component.text("FixedName")));
  check(score.isScoreSet() && main.getEntries().contains(player.getName()));
  score.setScore(-7);
  check(main.getObjective("compat") == objective && objective.getScore(player.getName()).getScore() == -7);
  Team team = main.registerNewTeam("team");
  team.setOption(Team.Option.NAME_TAG_VISIBILITY, Team.OptionStatus.NEVER);
  team.addEntry(player.getName());
 }
 private static void setPrivateBoard(Player player) {
  Scoreboard personal = Bukkit.getScoreboardManager().getNewScoreboard();
  Objective objective = personal.registerNewObjective("compat", "dummy", "§aPersonalTitle");
  objective.setDisplaySlot(DisplaySlot.SIDEBAR);
  objective.getScore("PersonalEntry").setScore(5);
  player.setScoreboard(personal);
  check(player.getScoreboard() == personal && objective.getDisplaySlot() == DisplaySlot.SIDEBAR);
  objective.setDisplayName("ChangedTitle");
  personal.resetScores("PersonalEntry");
  check(personal.getEntries().isEmpty() && !objective.getScore("PersonalEntry").isScoreSet());
  objective.getScore("PersonalEntry").setScore(6);
 }
 private static void clearMainBoard(Player player) {
  Scoreboard main = Bukkit.getScoreboardManager().getMainScoreboard();
  player.setScoreboard(main);
  Objective objective = main.getObjective("compat");
  Score score = objective.getScore(player.getName());
  check(score.isScoreSet() && score.getScore() == -7);
  score.resetScore();
  main.getTeam("team").unregister();
  objective.unregister();
  check(main.getObjective("compat") == null && main.getEntries().isEmpty());
 }
 @Override public boolean onCommand(CommandSender sender,Command command,String label,String[] args){
  if(!(sender instanceof Player)||args.length!=1)return false;
  Player player=(Player)sender;
  if(args[0].equals("info")){
   sender.sendMessage("PlayerInfo "+player.getPing()+" "+Bukkit.getMaxPlayers()+" "+getServer().getMaxPlayers());
   return true;
  }
  if (args[0].equals("metadata")) {
   org.bukkit.OfflinePlayer online = player;
   check(online.getPlayer().equals(player));
   org.bukkit.command.ConsoleCommandSender console = Bukkit.getConsoleSender();
   check(console == getServer().getConsoleSender() && console.isOp());
   console.sendMessage("ConsoleMetadata " + getDescription().getFullName());
   sender.sendMessage("Metadata checked");
   return true;
  }
  if (args[0].equals("minitext")) {
   Component parsed = MiniMessage.miniMessage().deserialize(
       "<green>Head <bold>bold</bold></green><br><gradient:#ff0000:#0000ff>A☃Z</gradient>\\<red>");
   LegacyComponentSerializer serializer = LegacyComponentSerializer.builder().character('&')
       .hexColors().useUnusualXRepeatedCharacterHexFormat().build();
   check(serializer.serialize(parsed).equals("&aHead &lbold&r\n&x&f&f&0&0&0&0A&x&8&0&0&0&8&0☃&x&0&0&0&0&f&fZ&r<red>"));
   player.sendMessage(parsed);
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
  switch (args[0]) {
   case "race-presentation": checkRacePresentation(player); sender.sendMessage("Race presentation checked"); return true;
   case "board-main": setMainBoard(player); sender.sendMessage("Board main"); return true;
   case "board-private": setPrivateBoard(player); sender.sendMessage("Board private"); return true;
   case "board-clear": clearMainBoard(player); sender.sendMessage("Board cleared"); return true;
   default: break;
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
