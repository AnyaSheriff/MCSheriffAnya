package ru.mcsa.translator;
import org.bukkit.event.*;
import ru.mcsa.bukkit.RuntimeState;
public final class HandlerContracts {
 private static int checks;
 private static void check(boolean value){checks++;if(!value)throw new AssertionError("check "+checks);}
 private static final class Probe extends Event {}
 private static class Counter implements Listener {int calls;}
 private static final class EqualCounter extends Counter {
  public boolean equals(Object other){return other instanceof EqualCounter;}
  public int hashCode(){return 1;}
 }
 public static void main(String[] args){
  RuntimeState.dispatcher=(listener,event,priority)->{if(priority==2)((Counter)listener).calls++;};
  Counter first=new Counter(),second=new Counter();
  RuntimeState.manager.registerEvents(first,null);RuntimeState.manager.registerEvents(second,null);
  RuntimeState.fire(new Probe());check(first.calls==1&&second.calls==1);
  HandlerList.unregisterAll(first);RuntimeState.fire(new Probe());check(first.calls==1&&second.calls==2);
  HandlerList.unregisterAll(first);RuntimeState.fire(new Probe());check(first.calls==1&&second.calls==3);
  RuntimeState.manager.registerEvents(first,null);RuntimeState.manager.registerEvents(first,null);
  RuntimeState.fire(new Probe());check(first.calls==2&&second.calls==4);
  RuntimeState.manager.registerEvents(first,null);RuntimeState.fire(new Probe());check(first.calls==3&&second.calls==5);
  HandlerList.unregisterAll(first);HandlerList.unregisterAll(second);
  EqualCounter equalFirst=new EqualCounter(),equalSecond=new EqualCounter(),unregisteredEqual=new EqualCounter();
  RuntimeState.manager.registerEvents(equalFirst,null);RuntimeState.manager.registerEvents(equalSecond,null);
  RuntimeState.fire(new Probe());check(equalFirst.calls==1&&equalSecond.calls==1);
  HandlerList.unregisterAll(unregisteredEqual);RuntimeState.fire(new Probe());check(equalFirst.calls==2&&equalSecond.calls==2);
  HandlerList.unregisterAll(equalFirst);RuntimeState.fire(new Probe());check(equalFirst.calls==2&&equalSecond.calls==3);
  HandlerList.unregisterAll(equalFirst);RuntimeState.fire(new Probe());check(equalFirst.calls==2&&equalSecond.calls==4);
  RuntimeState.manager.registerEvents(equalFirst,null);RuntimeState.manager.registerEvents(equalFirst,null);
  RuntimeState.fire(new Probe());check(equalFirst.calls==3&&equalSecond.calls==5);
  HandlerList.unregisterAll(equalFirst);HandlerList.unregisterAll(equalSecond);
  Counter priorities=new Counter();RuntimeState.manager.registerEvents(priorities,null);
  RuntimeState.dispatcher=(listener,event,priority)->{if(priority==0||priority==5)((Counter)listener).calls++;};
  RuntimeState.fire(new Probe());check(priorities.calls==2);
  HandlerList.unregisterAll(priorities);RuntimeState.fire(new Probe());check(priorities.calls==2);
  RuntimeState.manager.registerEvents(priorities,null);RuntimeState.fire(new Probe());check(priorities.calls==4);
  HandlerList.unregisterAll(priorities);
  Counter self=new Counter(),survivor=new Counter();
  RuntimeState.manager.registerEvents(self,null);RuntimeState.manager.registerEvents(survivor,null);
  RuntimeState.dispatcher=(listener,event,priority)->{if(priority==2){((Counter)listener).calls++;if(listener==self)HandlerList.unregisterAll(self);}};
  RuntimeState.fire(new Probe());check(self.calls==1&&survivor.calls==1);
  RuntimeState.fire(new Probe());check(self.calls==1&&survivor.calls==2);
  RuntimeState.manager.registerEvents(self,null);RuntimeState.manager.registerEvents(self,null);
  RuntimeState.fire(new Probe());check(self.calls==2&&survivor.calls==3);
  RuntimeState.fire(new Probe());check(self.calls==2&&survivor.calls==4);
  HandlerList.unregisterAll(survivor);
  System.out.println("PASS: "+checks+" HandlerList listener contracts");
 }
}
