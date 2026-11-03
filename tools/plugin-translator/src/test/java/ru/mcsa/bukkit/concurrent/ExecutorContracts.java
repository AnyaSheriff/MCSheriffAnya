package ru.mcsa.bukkit.concurrent;
import java.util.*;
import java.util.concurrent.*;
public final class ExecutorContracts {
 private static void check(boolean value){if(!value)throw new AssertionError();}
 public static void main(String[] args)throws Exception{
  ArrayDeque<Runnable> ticks=new ArrayDeque<>();List<Integer> order=new ArrayList<>();ExecutorService executor=Executors.ordered(ticks::add);
  executor.execute(()->{order.add(1);executor.execute(()->order.add(3));});
  Future<Integer> value=executor.submit(()->{order.add(2);return 42;});
  check(order.isEmpty()&&ticks.size()==1&&!value.isDone());
  boolean blocked=false;try{value.get();}catch(IllegalStateException expected){blocked=true;}check(blocked);
  ticks.remove().run();check(order.equals(List.of(1,2))&&value.get()==42&&ticks.size()==1);
  ticks.remove().run();check(order.equals(List.of(1,2,3)));
  Future<?> cancelled=executor.submit(()->order.add(99));check(cancelled.cancel(true));ticks.remove().run();check(!order.contains(99)&&cancelled.isCancelled());
  Future<?> failed=executor.submit(()->{throw new IllegalArgumentException("expected");});ticks.remove().run();boolean captured=false;try{failed.get();}catch(ExecutionException expected){captured=expected.getCause() instanceof IllegalArgumentException;}check(captured);
  executor.execute(()->order.add(4));executor.shutdown();check(!executor.isTerminated());check(executor.awaitTermination(10,TimeUnit.SECONDS));check(order.equals(List.of(1,2,3,4)));
  boolean rejected=false;try{executor.execute(()->{});}catch(RejectedExecutionException expected){rejected=true;}check(rejected);
  ExecutorService bounded=Executors.ordered(ticks::add);for(int i=0;i<256;i++)bounded.execute(()->{});rejected=false;try{bounded.execute(()->{});}catch(RejectedExecutionException expected){rejected=true;}check(rejected&&bounded.shutdownNow().size()==256&&bounded.isTerminated());
  ticks.clear();order.clear();ExecutorService[] pair=Executors.pair(ticks::add);pair[0].execute(()->order.add(1));pair[1].execute(()->order.add(2));pair[0].execute(()->order.add(3));check(ticks.size()==1);ticks.remove().run();check(order.equals(List.of(1,2,3)));
  rejected=false;try{Executors.newFixedThreadPool(2);}catch(IllegalArgumentException expected){rejected=true;}check(rejected);
  System.out.println("PASS: 15 контрактов очереди executor (порядок, такты, future, отмена, shutdown, лимит)");
 }
}
