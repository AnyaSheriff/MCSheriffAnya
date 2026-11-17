package ru.mcsa.bukkit;
import java.util.*;
import org.bukkit.boss.*;
import org.bukkit.entity.Player;

// Только собственный WIT presentation: id локален компоненту, пакеты создаёт хозяин.
public final class RemoteBossBar implements BossBar {
 private static long nextId;
 private static final Map<UUID,Set<RemoteBossBar>> attached=new HashMap<>();
 private final String id;
 private final Map<UUID,Player> players=new LinkedHashMap<>();
 private String title,titleJson;
 private BarColor color;
 private BarStyle style;
 private double progress=1.0;
 private boolean visible=true;
 public RemoteBossBar(String title,BarColor color,BarStyle style,BarFlag... flags){
  Objects.requireNonNull(flags,"Массив флагов");
  if(flags.length!=0)throw new UnsupportedOperationException("Флаги BossBar пока не поддерживаются");
  this.title=title==null?"":title;this.titleJson=titleJson(this.title);
  this.color=Objects.requireNonNull(color,"Цвет полосы");this.style=Objects.requireNonNull(style,"Деления полосы");
  if(nextId==Long.MAX_VALUE)throw new IllegalStateException("Исчерпаны идентификаторы полос");
  id="bukkit-bar-"+(++nextId);
 }
 private static String titleJson(String value){
  String json=Text.json(Text.legacy(value,'\u00a7'));
  if(Strings.utf8(json).length>16384)throw new IllegalArgumentException("Заголовок полосы превышает 16 КиБ");
  return json;
 }
 private static String hostStyle(BarStyle value){return value==BarStyle.SOLID?"progress":"notched_"+value.name().substring("SEGMENTED_".length());}
 private void send(Player player,String json,double amount,BarColor color,BarStyle style){
  if(!Host.yes(162,player.getUniqueId().toString(),id,json,Double.toString(amount),Ascii.lower(color.name()),hostStyle(style)))
   throw new IllegalStateException("Полоса босса отклонена хозяином: игрок не готов или превышен лимит 32 полос");
 }
 private void remove(Player player){if(!Host.yes(163,player.getUniqueId().toString(),id))throw new IllegalStateException("Скрытие полосы босса отклонено хозяином");}
 // Реестр содержит только полосы с участниками, а не все когда-либо созданные объекты.
 private void detach(UUID uuid){players.remove(uuid);Set<RemoteBossBar> bars=attached.get(uuid);if(bars!=null){bars.remove(this);if(bars.isEmpty())attached.remove(uuid);}}
 public static void leave(Player player){UUID uuid=player.getUniqueId();Set<RemoteBossBar> bars=attached.remove(uuid);if(bars!=null)for(RemoteBossBar bar:bars)bar.players.remove(uuid);}
 // Хозяин тоже освобождает оформление при выходе. Отсутствующий quit не оживляет обёртку.
 private void prune(){Set<UUID> online=new HashSet<>();for(Player player:org.bukkit.Bukkit.getOnlinePlayers())online.add(player.getUniqueId());for(UUID uuid:new ArrayList<>(players.keySet()))if(!online.contains(uuid))detach(uuid);}
 private void update(String json,double amount,BarColor color,BarStyle style){prune();if(visible)for(Player player:players.values())send(player,json,amount,color,style);}
 public String getTitle(){return title;}
 public void setTitle(String value){value=value==null?"":value;String json=titleJson(value);update(json,progress,color,style);title=value;titleJson=json;}
 public BarColor getColor(){return color;}
 public void setColor(BarColor value){Objects.requireNonNull(value,"Цвет полосы");update(titleJson,progress,value,style);color=value;}
 public BarStyle getStyle(){return style;}
 public void setStyle(BarStyle value){Objects.requireNonNull(value,"Деления полосы");update(titleJson,progress,color,value);style=value;}
 public double getProgress(){return progress;}
 public void setProgress(double value){if(!Double.isFinite(value)||value<0||value>1)throw new IllegalArgumentException("Заполнение полосы: конечное число 0..1");update(titleJson,value,color,style);progress=value;}
 public void addPlayer(Player player){
  Objects.requireNonNull(player,"Игрок");prune();UUID uuid=player.getUniqueId();if(players.containsKey(uuid))return;
  if(!player.isOnline())throw new IllegalArgumentException("Игрок полосы отсутствует на сервере");
  if(players.size()>=1024)throw new IllegalStateException("Лимит участников полосы: 1024");
  Set<RemoteBossBar> bars=attached.get(uuid);if(bars!=null&&bars.size()>=1024)throw new IllegalStateException("Лимит назначенных игроку полос: 1024");
  if(visible)send(player,titleJson,progress,color,style);players.put(uuid,player);
  if(bars==null){bars=new HashSet<>();attached.put(uuid,bars);}bars.add(this);
 }
 public void removePlayer(Player player){Objects.requireNonNull(player,"Игрок");prune();Player old=players.get(player.getUniqueId());if(old==null)return;if(visible)remove(old);detach(player.getUniqueId());}
 public void removeAll(){prune();for(Player player:new ArrayList<>(players.values()))removePlayer(player);}
 // Снимок нельзя менять снаружи; разные Java-обёртки одного UUID не дублируют зрителя.
 public List<Player> getPlayers(){prune();return Collections.unmodifiableList(new ArrayList<>(players.values()));}
 public boolean isVisible(){return visible;}
 public void setVisible(boolean value){
  prune();if(visible==value)return;List<Player> changed=new ArrayList<>(players.size());
  try{
   for(Player player:players.values()){
    if(value)send(player,titleJson,progress,color,style);else remove(player);
    changed.add(player);
   }
  }catch(RuntimeException|Error failure){
   // В одном callback хозяин не исполняет другие плагины: освобождённые слоты
   // при откате скрытия остаются свободными. Отказ показа не оставляет «призрак».
   for(int i=changed.size()-1;i>=0;i--)try{
    if(value)remove(changed.get(i));else send(changed.get(i),titleJson,progress,color,style);
   }catch(RuntimeException|Error rollbackFailure){failure.addSuppressed(rollbackFailure);}
   throw failure;
  }
  visible=value;
 }
}
