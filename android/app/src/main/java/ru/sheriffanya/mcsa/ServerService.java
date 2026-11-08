package ru.sheriffanya.mcsa;

import android.annotation.SuppressLint;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.os.PowerManager;
import java.io.BufferedWriter;
import java.io.File;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.RejectedExecutionException;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.TimeUnit;

/** Исполняется только бинарник из read-only nativeLibraryDir установленного APK. */
public final class ServerService extends Service {
    static final String START="ru.sheriffanya.mcsa.START", STOP="ru.sheriffanya.mcsa.STOP", COMMAND="ru.sheriffanya.mcsa.COMMAND";
    private static final String CHANNEL="server";
    private static final int NOTIFICATION=1;
    private final Handler main=new Handler(Looper.getMainLooper());
    private final ThreadPoolExecutor writer=new ThreadPoolExecutor(1,1,0L,TimeUnit.MILLISECONDS,new ArrayBlockingQueue<>(32));
    private volatile BufferedWriter input;
    private volatile boolean stopRequested;
    private final Object stdinLock=new Object();
    private PowerManager.WakeLock wakeLock;
    private boolean launched;
    private boolean runtimeFinished, destroyed;
    private CoreUpdateStore coreStore;private CoreUpdateStore.Core selectedCore;private boolean coreReady;private String actualCoreBuild;

    @Override public IBinder onBind(Intent intent) { return null; }
    @Override public int onStartCommand(Intent intent,int flags,int startId) {
        String action=intent==null?"":intent.getAction();
        if(START.equals(action)) start();
        else if(STOP.equals(action)) requestStop();
        else if(COMMAND.equals(action)) {
            String value=intent.getStringExtra("command");
            if("stop".equals(value==null?null:value.trim())) requestStop(); else command(value);
        }
        if(!launched) stopSelf();
        // ОС не должна неожиданно перезапускать сервер после остановки пользователем.
        return START_NOT_STICKY;
    }
    @SuppressLint("WakelockTimeout")
    private void start() {
        synchronized(HostState.LOCK) {
            if(launched || !HostState.idle()) return;
            try {
                new BackupStore(getFilesDir()).recoverForLaunch();
                File root=new File(getFilesDir(),"server");
                ServerSettings settings=new ServerSettings(root); settings.save(settings.values());
                File executable=new File(getApplicationInfo().nativeLibraryDir,"libmcsa.so");
                if(!executable.isFile() || !executable.canExecute()) throw new IOException("В APK нет подходящего ядра для этого устройства");
                HostState.phase=HostState.Phase.STARTING; HostState.message="Запуск сервера…";
                showNotification("Запуск сервера…");
                wakeLock=((PowerManager)getSystemService(POWER_SERVICE)).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK,"mcsa:server");
                wakeLock.setReferenceCounted(false); wakeLock.acquire();
                launched=true;
                new Thread(()->run(executable,root),"mcsa-runtime").start();
            } catch(Exception error) {
                HostState.phase=HostState.Phase.STOPPED; HostState.message="Не удалось запустить: "+error.getMessage();
                HostState.append(HostState.message+"\n"); release(); stopForeground(true);
            }
        }
    }
    private void run(File executable,File root) {
        Process process=null;CoreStatsClient monitor=null;File apiSocket=CoreStatsClient.socket(this);
        try {
            new BackupStore(getFilesDir()).cleanupRecovery();
            if(stopRequested) return;
            if(BuildConfig.CORE_HOST_API==1) {
                coreStore=new CoreUpdateStore(this);selectedCore=coreStore.prepareLaunch();
                synchronized(HostState.LOCK) { HostState.coreVersion=selectedCore.version;HostState.coreUpdateRevision++; }
            }
            if(stopRequested) return;
            ProcessBuilder builder=new ProcessBuilder(executable.getAbsolutePath()).directory(root).redirectErrorStream(true);
            if(selectedCore!=null) builder.environment().put("MCSA_CORE_LIBRARY",selectedCore.library.getPath());
            builder.environment().put("NO_COLOR","1"); builder.environment().put("TERM","dumb");
            if(apiSocket.exists()&&!apiSocket.delete()) throw new IOException("Не удалось подготовить локальный API");
            String session=java.util.UUID.randomUUID().toString();
            builder.environment().put("MCSA_APP_API_SOCKET",apiSocket.getPath());builder.environment().put("MCSA_APP_API_SESSION",session);
            synchronized(HostState.LOCK) { HostState.apiSession=session;HostState.stats=null;HostState.statsError=null; }
            process=builder.start();monitor=new CoreStatsClient(this,apiSocket,session);monitor.start();
            input=new BufferedWriter(new OutputStreamWriter(process.getOutputStream(),StandardCharsets.UTF_8));
            if(stopRequested) writeLine("stop");
            try(InputStreamReader reader=new InputStreamReader(process.getInputStream(),StandardCharsets.UTF_8)) {
                char[] buffer=new char[2048]; int n; StringBuilder line=new StringBuilder();
                while((n=reader.read(buffer))!=-1) {
                    for(int i=0;i<n;i++) {
                        char ch=buffer[i];
                        if(ch=='\n') { received(line.toString()); line.setLength(0); }
                        else if(ch!='\r') {
                            line.append(ch);
                            if(line.length()>=8192) { received(line.toString()); line.setLength(0); }
                        }
                    }
                }
                if(line.length()>0) received(line.toString());
            }
            int exit=process.waitFor();
            synchronized(HostState.LOCK) { HostState.message=exit==0?"Сервер остановлен":"Сервер завершился с кодом "+exit+". Открой консоль."; }
        } catch(Exception error) {
            synchronized(HostState.LOCK) { HostState.message="Ошибка сервера: "+error.getMessage(); }
            HostState.append(HostState.message+"\n");
            // При ошибке чтения просим сохранить мир, не объявляя процесс остановленным раньше выхода.
            if(process!=null) {
                try { writeLine("stop"); process.waitFor(); }
                catch(Exception ignored) { Thread.currentThread().interrupt(); }
            }
        } finally {
            if(coreStore!=null&&selectedCore!=null&&!coreReady&&coreStore.failed(selectedCore,stopRequested)) {
                synchronized(HostState.LOCK) { HostState.message="Новое ядро не запустилось. Прежняя версия возвращена."; }
            }
            if(monitor!=null) monitor.close();
            synchronized(HostState.LOCK) { HostState.apiSession=null;HostState.stats=null;HostState.statsError=null; }
            if(apiSocket.exists()) apiSocket.delete();
            input=null;
            release(); main.post(()->{
                runtimeFinished=true;
                if(destroyed) { synchronized(HostState.LOCK) { HostState.phase=HostState.Phase.STOPPED; } }
                else { stopForeground(true); stopSelf(); }
            });
        }
    }
    private void received(String line) {
        HostState.append(line+"\n");
        if(line.startsWith("MCSA_CORE_BUILD=")) actualCoreBuild=line.substring("MCSA_CORE_BUILD=".length());
        if(line.contains("Готово (")) {
            if(coreStore!=null&&selectedCore!=null) {
                try { coreStore.ready(selectedCore,actualCoreBuild);coreReady=true; }
                catch(Exception invalid) { HostState.append("Проверка запуска ядра не пройдена: "+invalid.getMessage()+"\n");send("stop");return; }
            }
            synchronized(HostState.LOCK) {
                if(HostState.phase==HostState.Phase.STARTING) { HostState.phase=HostState.Phase.RUNNING; HostState.message="Сервер работает"; }
            }
            main.post(()->{ if(!stopRequested) showNotification("Сервер работает"); });
        }
    }
    private void requestStop() {
        if(!launched) return;
        stopRequested=true;
        synchronized(HostState.LOCK) { HostState.phase=HostState.Phase.STOPPING; HostState.message="Сохраняем мир и останавливаем сервер…"; }
        showNotification("Сохранение мира…");
        writer.getQueue().clear();
        if(input!=null) send("stop");
    }
    private void command(String value) {
        if(value==null || value.trim().isEmpty() || value.length()>1024 || value.indexOf('\n')>=0 || value.indexOf('\r')>=0 || value.indexOf('\0')>=0) return;
        synchronized(HostState.LOCK) { if(HostState.phase!=HostState.Phase.RUNNING) return; }
        HostState.append("› "+value+"\n"); send(value);
    }
    private void send(String value) {
        try {
            writer.execute(()->{
                try { writeLine(value); }
                catch(IOException error) { HostState.append("Не удалось передать команду: "+error.getMessage()+"\n"); }
            });
        } catch(RejectedExecutionException full) { HostState.append("Очередь команд занята. Повтори после обработки предыдущих.\n"); }
    }
    private void writeLine(String value) throws IOException {
        synchronized(stdinLock) { BufferedWriter target=input; if(target!=null) { target.write(value); target.newLine(); target.flush(); } }
    }
    private void showNotification(String text) {
        NotificationManager manager=(NotificationManager)getSystemService(NOTIFICATION_SERVICE);
        if(Build.VERSION.SDK_INT>=26) manager.createNotificationChannel(new NotificationChannel(CHANNEL,"Работа сервера",NotificationManager.IMPORTANCE_LOW));
        PendingIntent open=PendingIntent.getActivity(this,0,new Intent(this,MainActivity.class),PendingIntent.FLAG_UPDATE_CURRENT|PendingIntent.FLAG_IMMUTABLE);
        PendingIntent stop=PendingIntent.getService(this,1,new Intent(this,ServerService.class).setAction(STOP),PendingIntent.FLAG_UPDATE_CURRENT|PendingIntent.FLAG_IMMUTABLE);
        Notification.Builder builder=Build.VERSION.SDK_INT>=26?new Notification.Builder(this,CHANNEL):new Notification.Builder(this);
        Notification notification=builder.setSmallIcon(R.drawable.project_logo_monochrome).setContentTitle("MCSheriffAnya").setContentText(text).setContentIntent(open).setOngoing(true)
                .addAction(new Notification.Action.Builder(R.drawable.ic_server,"Остановить",stop).build()).build();
        if(Build.VERSION.SDK_INT>=34) startForeground(NOTIFICATION,notification,ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE);
        else startForeground(NOTIFICATION,notification);
    }
    private void release() { if(wakeLock!=null && wakeLock.isHeld()) wakeLock.release(); }
    @Override public void onDestroy() {
        // Уход с экрана сам по себе сервис не уничтожает. Если сервис всё же остановлен — штатный stop.
        destroyed=true;
        if(runtimeFinished) { synchronized(HostState.LOCK) { HostState.phase=HostState.Phase.STOPPED; } }
        else if(launched && !HostState.idle()) {
            stopRequested=true;
            synchronized(HostState.LOCK) { HostState.phase=HostState.Phase.STOPPING; }
            writer.getQueue().clear();
            if(input!=null) send("stop");
        }
        writer.shutdown(); release(); super.onDestroy();
    }
}
