package ru.mcsa.bukkit.concurrent;
public interface ExecutorService extends Executor,AutoCloseable {
 default void close(){shutdown();try{if(!awaitTermination(Long.MAX_VALUE,java.util.concurrent.TimeUnit.NANOSECONDS))throw new IllegalStateException("Очередь ещё выполняется; блокирующее ожидание запрещено");}catch(InterruptedException e){throw new IllegalStateException("Ожидание прервано",e);}}
 void shutdown();java.util.List<Runnable> shutdownNow();boolean isShutdown();boolean isTerminated();boolean awaitTermination(long timeout,java.util.concurrent.TimeUnit unit) throws InterruptedException;
 <T> Future<T> submit(Callable<T> task);Future<?> submit(Runnable task);<T> Future<T> submit(Runnable task,T result);
}
