package ru.mcsa.bukkit;
import java.util.*;

// PDC сущностей каждого компонента хранится только в его data/, по UUID.
public final class EntityDataStore {
 private static final Map<UUID,DataContainer> entries=new LinkedHashMap<>();
 private static final Map<String,Persistent> dirty=new LinkedHashMap<>();private static boolean scheduled;
 private static final class Persistent extends DataContainer {
  final String file;boolean ready;
  Persistent(UUID uuid){file="entity-"+uuid+".pdc.json";if(Host.yes(34,file))DataContainer.parse(Host.text(8,file)).copyTo(this,true);ready=true;}
  @Override protected void changed(){if(!ready)return;if(dirty.size()>=256&&!dirty.containsKey(file))throw new IllegalStateException("Лимит ожидающих записей PDC сущностей");dirty.put(file,this);if(!scheduled){scheduled=true;try{RuntimeState.scheduler.runTask(RuntimeState.plugin,()->{scheduled=false;flush();});}catch(RuntimeException e){scheduled=false;throw e;}}}
 }
 public static DataContainer get(UUID uuid){return entries.computeIfAbsent(Objects.requireNonNull(uuid),Persistent::new);}
 public static void flush(){Map<String,Persistent> pending=new LinkedHashMap<>(dirty);dirty.clear();try{for(Map.Entry<String,Persistent> entry:pending.entrySet())Host.call(9,entry.getKey(),entry.getValue().serialize());}catch(RuntimeException e){dirty.putAll(pending);throw e;}}
}
