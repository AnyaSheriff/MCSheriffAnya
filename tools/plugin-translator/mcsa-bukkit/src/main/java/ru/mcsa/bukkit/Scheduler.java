package ru.mcsa.bukkit;
import org.bukkit.scheduler.*;import org.bukkit.plugin.Plugin;
public final class Scheduler implements BukkitScheduler {
 private final java.util.Map<Integer,Task> tasks=new java.util.LinkedHashMap<>();private int next;
 private final class Task implements BukkitTask {int id,key;Plugin owner;Runnable run;boolean repeat,cancelled;public int getTaskId(){return id;}public Plugin getOwner(){return owner;}public boolean isSync(){return true;}public boolean isCancelled(){return cancelled;}public void cancel(){Host.call(7,""+id);tasks.remove(key);cancelled=true;}}
 private BukkitTask add(Plugin p,Runnable r,long d,long period){if(d<0||d>Integer.MAX_VALUE||period>Integer.MAX_VALUE||period==0)throw new IllegalArgumentException("Неверный срок задачи");Task t=new Task();t.key=++next;t.owner=p;t.run=r;t.repeat=period>0;t.id=Integer.parseInt(Host.text(period<0?5:6,""+d,""+period,""+t.key));if(t.id==0)throw new IllegalStateException("Лимит задач");tasks.put(t.key,t);return t;}
 public BukkitTask runTask(Plugin p,Runnable r){return add(p,r,0,-1);}public BukkitTask runTaskLater(Plugin p,Runnable r,long d){return add(p,r,d,-1);}public BukkitTask runTaskTimer(Plugin p,Runnable r,long d,long t){if(t<1)throw new IllegalArgumentException("Период меньше 1");return add(p,r,d,t);}
 public int scheduleSyncDelayedTask(Plugin p,Runnable r,long d){return runTaskLater(p,r,d).getTaskId();}public int scheduleSyncRepeatingTask(Plugin p,Runnable r,long d,long t){return runTaskTimer(p,r,d,t).getTaskId();}
 public void cancelTask(int id){for(Task t:new java.util.ArrayList<>(tasks.values()))if(t.id==id)t.cancel();}public void cancelTasks(Plugin p){for(Task t:new java.util.ArrayList<>(tasks.values()))if(t.owner==p)t.cancel();}
 public void fire(int key){Task t=tasks.get(key);if(t!=null){if(!t.repeat)tasks.remove(key);t.run.run();}}
}
