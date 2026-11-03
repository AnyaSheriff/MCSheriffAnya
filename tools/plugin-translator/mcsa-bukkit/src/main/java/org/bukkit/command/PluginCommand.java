package org.bukkit.command;
public final class PluginCommand extends Command {
 private TabCompleter completer;private String permission="",permissionMessage="У вас нет прав на эту команду.";
 public Command setPermissionMessage(String message){permissionMessage=message==null?"У вас нет прав на эту команду.":message;return this;}
 public void setPermission(String value){permission=value==null?"":value;}
 public boolean testPermissionSilent(CommandSender sender){if(permission.isEmpty())return true;for(String part:permission.split(";"))if(sender.hasPermission(part))return true;return false;}
 public void setTabCompleter(TabCompleter value){completer=value;}
 public java.util.List<String> tabComplete(CommandSender sender,String label,String[] args){if(!testPermissionSilent(sender))return java.util.Collections.emptyList();TabCompleter value=completer;if(value==null&&executor instanceof TabCompleter)value=(TabCompleter)executor;java.util.List<String> result=value==null?null:value.onTabComplete(sender,this,label,args);if(result!=null)return result;java.util.List<String> names=new java.util.ArrayList<>();String prefix=args.length==0?"":args[args.length-1];for(org.bukkit.entity.Player p:org.bukkit.Bukkit.getOnlinePlayers())if(p.getName().startsWith(prefix))names.add(p.getName());return names;}
 private CommandExecutor executor; public PluginCommand(String n,CommandExecutor e){super(n);executor=e;}
 public void setExecutor(CommandExecutor e){executor=e;} public CommandExecutor getExecutor(){return executor;}
 public boolean execute(CommandSender s,String l,String[] a){if(!testPermissionSilent(s)){if(!permissionMessage.isEmpty())s.sendMessage(permissionMessage.replace("<permission>",permission));return true;}return executor.onCommand(s,this,l,a);}
}
