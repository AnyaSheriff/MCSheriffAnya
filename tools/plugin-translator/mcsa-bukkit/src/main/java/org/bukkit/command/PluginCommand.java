package org.bukkit.command;
public final class PluginCommand extends Command {
 private TabCompleter completer;
 public void setTabCompleter(TabCompleter value){completer=value;}
 public java.util.List<String> tabComplete(CommandSender sender,String label,String[] args){TabCompleter value=completer;if(value==null&&executor instanceof TabCompleter)value=(TabCompleter)executor;java.util.List<String> result=value==null?null:value.onTabComplete(sender,this,label,args);if(result!=null)return result;java.util.List<String> names=new java.util.ArrayList<>();String prefix=args.length==0?"":args[args.length-1];for(org.bukkit.entity.Player p:org.bukkit.Bukkit.getOnlinePlayers())if(p.getName().startsWith(prefix))names.add(p.getName());return names;}
 private CommandExecutor executor; public PluginCommand(String n,CommandExecutor e){super(n);executor=e;}
 public void setExecutor(CommandExecutor e){executor=e;} public CommandExecutor getExecutor(){return executor;}
 public boolean execute(CommandSender s,String l,String[] a){return executor.onCommand(s,this,l,a);}
}
