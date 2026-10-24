package ru.mcsa.bukkit;
import org.bukkit.scoreboard.*;import org.bukkit.entity.Player;import net.kyori.adventure.text.Component;import net.kyori.adventure.text.format.NamedTextColor;
public final class MainScoreboard implements Scoreboard {
 public static final MainScoreboard INSTANCE=new MainScoreboard();
 private final java.util.Map<String,MainTeam> teams=new java.util.LinkedHashMap<>();
 public Team getTeam(String name){return teams.get(java.util.Objects.requireNonNull(name));}
 public java.util.Set<Team> getTeams(){return java.util.Collections.unmodifiableSet(new java.util.LinkedHashSet<Team>(teams.values()));}
 public Team getEntryTeam(String entry){for(MainTeam team:teams.values())if(team.hasEntry(entry))return team;return null;}
 public Team registerNewTeam(String name){if(name==null||name.isEmpty()||name.length()>128||teams.containsKey(name))throw new IllegalArgumentException("Недопустимое или занятое имя команды");if(teams.size()>=256)throw new IllegalStateException("Лимит команд: 256");MainTeam team=new MainTeam(name);teams.put(name,team);team.send();return team;}
 public void sync(Player viewer){for(MainTeam team:teams.values())team.send(viewer);}
 private final class MainTeam implements Team {
  private final String name;private final java.util.Set<String> entries=new java.util.LinkedHashSet<>();private Component prefix=Component.empty(),suffix=Component.empty();private NamedTextColor color;private boolean registered=true;
  MainTeam(String name){this.name=name;}
  private void check(){if(!registered)throw new IllegalStateException("Команда удалена");}
  public String getName(){check();return name;}public Scoreboard getScoreboard(){return registered?MainScoreboard.this:null;}
  public boolean hasEntry(String entry){check();return entries.contains(java.util.Objects.requireNonNull(entry));}
  public java.util.Set<String> getEntries(){check();return java.util.Collections.unmodifiableSet(new java.util.LinkedHashSet<>(entries));}
  public void addEntry(String entry){check();if(entry==null||entry.length()>128)throw new IllegalArgumentException("Недопустимая запись команды");if(entries.contains(entry))return;if(entries.size()>=1024)throw new IllegalStateException("Лимит записей команды: 1024");Team old=getEntryTeam(entry);if(old!=null)old.removeEntry(entry);entries.add(entry);send();}
  public boolean removeEntry(String entry){check();boolean removed=entries.remove(java.util.Objects.requireNonNull(entry));if(removed)send();return removed;}
  public void prefix(Component value){check();prefix=java.util.Objects.requireNonNull(value);send();}public Component prefix(){check();return prefix;}
  public void suffix(Component value){check();suffix=java.util.Objects.requireNonNull(value);send();}public Component suffix(){check();return suffix;}
  public void color(NamedTextColor value){check();color=value;send();}public NamedTextColor color(){check();return color;}
  public void unregister(){check();teams.remove(name);registered=false;for(Player viewer:org.bukkit.Bukkit.getOnlinePlayers())if(!Host.yes(59,viewer.getUniqueId().toString(),name))throw new IllegalStateException("Удаление команды отклонено хозяином");}
  void send(){for(Player viewer:org.bukkit.Bukkit.getOnlinePlayers())send(viewer);}
  void send(Player viewer){java.util.List<String> args=new java.util.ArrayList<>();java.util.Collections.addAll(args,viewer.getUniqueId().toString(),name,Text.json(prefix),Text.json(suffix),color==null?"reset":color.toString(),"always");args.addAll(entries);if(!Host.yes(58,args.toArray(new String[0])))throw new IllegalStateException("Команда scoreboard отклонена хозяином");}
 }
}
