package ru.mcsa.bukkit.concurrent;
public interface Future<V> {boolean cancel(boolean mayInterruptIfRunning);boolean isCancelled();boolean isDone();V get() throws java.util.concurrent.ExecutionException;V get(long timeout,java.util.concurrent.TimeUnit unit) throws java.util.concurrent.ExecutionException,java.util.concurrent.TimeoutException;}
