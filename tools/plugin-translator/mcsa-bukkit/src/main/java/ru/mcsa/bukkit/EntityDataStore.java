package ru.mcsa.bukkit;
import java.util.*;

// PDC сущностей каждого компонента хранится только в его data/, по UUID.
public final class EntityDataStore {
 private static final Map<UUID,DataContainer> entries=new LinkedHashMap<>();
 private static final Map<String,Persistent> dirty=new LinkedHashMap<>();private static boolean scheduled;
 private static final class Persistent extends DataContainer {
  final String file;boolean ready;
  Persistent(UUID uuid){file="entity-"+uuid+".pdc.json";if(Host.yes(34,file))DataContainer.parse(Host.text(8,file)).copyTo(this,true);ready=true;}
  @Override protected void changed(){if(!ready)return;if(dirty.size()>=256&&!dirty.containsKey(file))throw new IllegalStateException("Лимит ожидающих записей PDC сущностей");dirty.put(file,this);schedule();}
 }
 public static DataContainer get(UUID uuid){return entries.computeIfAbsent(Objects.requireNonNull(uuid),Persistent::new);}
 // Одна запись на задачу: появление группы мобов не задерживает один callback всей пачкой.
 private static void schedule(){if(scheduled||dirty.isEmpty())return;scheduled=true;try{RuntimeState.scheduler.runTask(RuntimeState.plugin,()->{scheduled=false;flushOne();schedule();});}catch(RuntimeException e){scheduled=false;throw e;}}
 private static void flushOne(){if(dirty.isEmpty())return;Map.Entry<String,Persistent> entry=dirty.entrySet().iterator().next();dirty.remove(entry.getKey());try{Host.call(9,entry.getKey(),entry.getValue().serialize());}catch(RuntimeException e){dirty.put(entry.getKey(),entry.getValue());throw e;}}
 public static void flush(){while(!dirty.isEmpty())flushOne();}
}
