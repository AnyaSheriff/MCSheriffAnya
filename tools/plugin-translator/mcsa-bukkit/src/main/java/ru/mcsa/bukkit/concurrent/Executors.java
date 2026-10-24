package ru.mcsa.bukkit.concurrent;
import java.util.*;
import java.util.concurrent.*;
import ru.mcsa.bukkit.RuntimeState;

// Очередь компонента обслуживает хозяин через обычный планировщик между тактами.
public final class Executors {
 private static final Queue DEFAULT=new Queue(task->RuntimeState.scheduler.runTask(RuntimeState.plugin,task));
 public static ExecutorService newSingleThreadExecutor(){return new Ordered(DEFAULT);}
 public static ExecutorService newFixedThreadPool(int threads){if(threads!=1)throw new IllegalArgumentException("Параллельные потоки запрещены; разрешена только упорядоченная очередь одного исполнителя");return newSingleThreadExecutor();}
 private static final class Result<T> implements Future<T>,Runnable {
  final Callable<T> task;boolean done,cancelled;T value;Exception failure;
  Result(Callable<T> task){this.task=Objects.requireNonNull(task);}
  public void run(){if(done)return;try{value=task.call();}catch(Exception e){failure=e;}finally{done=true;}}
  public boolean cancel(boolean interrupt){if(done)return false;cancelled=true;done=true;return true;}
  public boolean isCancelled(){return cancelled;}public boolean isDone(){return done;}
  public T get()throws ExecutionException{if(!done)throw new IllegalStateException("Блокирующее ожидание запрещено: задача выполняется хозяином между тактами");if(cancelled)throw new CancellationException();if(failure!=null)throw new ExecutionException(failure);return value;}
  public T get(long timeout,TimeUnit unit)throws ExecutionException,TimeoutException{Objects.requireNonNull(unit);if(!done)throw new TimeoutException("Задача ещё в очереди хозяина");return get();}
 }
 static ExecutorService ordered(java.util.function.Consumer<Runnable> schedule){return new Ordered(new Queue(schedule));}
 static ExecutorService[] pair(java.util.function.Consumer<Runnable> schedule){Queue queue=new Queue(schedule);return new ExecutorService[]{new Ordered(queue),new Ordered(queue)};}
 private static final class Job {final Ordered owner;final Runnable task;final long number;Job(Ordered owner,Runnable task,long number){this.owner=owner;this.task=task;this.number=number;}}
 private static final class Queue {
  final java.util.function.Consumer<Runnable> host;final ArrayDeque<Job> jobs=new ArrayDeque<>();boolean scheduled,running;long next;
  Queue(java.util.function.Consumer<Runnable> host){this.host=host;}
  void schedule(){host.accept(()->{scheduled=false;drain(64);});scheduled=true;}
  void add(Ordered owner,Runnable task){if(jobs.size()>=256)throw new RejectedExecutionException("Очередь плагина заполнена");jobs.add(new Job(owner,task,++next));owner.pending++;if(!scheduled&&!running){try{schedule();}catch(RuntimeException e){jobs.removeLast();owner.pending--;throw e;}}}
  void drain(int budget){if(running)return;running=true;long boundary=next;try{while(budget-->0&&!jobs.isEmpty()&&jobs.peek().number<=boundary){Job job=jobs.remove();try{job.task.run();}catch(RuntimeException e){ru.mcsa.bukkit.Host.call(23,"Ошибка задачи очереди: "+e.getMessage());}finally{job.owner.pending--;}}}finally{running=false;if(!jobs.isEmpty()&&!scheduled)schedule();}}
 }
 private static final class Ordered implements ExecutorService {
  final Queue queue;boolean shutdown;int pending;Ordered(Queue queue){this.queue=queue;}
  public void execute(Runnable task){Objects.requireNonNull(task);if(shutdown)throw new RejectedExecutionException("Очередь закрыта");queue.add(this,task);}
  public void shutdown(){shutdown=true;}
  public List<Runnable> shutdownNow(){shutdown=true;List<Runnable> rest=new ArrayList<>();java.util.Iterator<Job> iterator=queue.jobs.iterator();while(iterator.hasNext()){Job job=iterator.next();if(job.owner==this){iterator.remove();pending--;rest.add(job.task);if(job.task instanceof Result)((Result<?>)job.task).cancel(false);}}return rest;}
  public boolean isShutdown(){return shutdown;}public boolean isTerminated(){return shutdown&&pending==0;}
  public boolean awaitTermination(long timeout,TimeUnit unit){Objects.requireNonNull(unit);if(timeout<0)throw new IllegalArgumentException("Отрицательное время");if(shutdown&&!queue.running&&timeout>0)queue.drain(256);return isTerminated();}
  public <T> Future<T> submit(Callable<T> task){Result<T> result=new Result<>(task);execute(result);return result;}
  public Future<?> submit(Runnable task){return submit(task,null);}
  public <T> Future<T> submit(Runnable task,T result){Objects.requireNonNull(task);return submit(()->{task.run();return result;});}
 }
}
