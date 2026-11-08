package ru.sheriffanya.mcsa;

import android.app.*;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.*;
import java.io.File;
import java.io.IOException;

/** Проверка/загрузка продолжается при закрытом экране; переключение только без сервера. */
public final class CoreUpdateService extends Service {
    static final String CHECK="mcsa.core.CHECK",INSTALL="mcsa.core.INSTALL",ROLLBACK="mcsa.core.ROLLBACK";
    private boolean working,finished,destroyed;private PowerManager.WakeLock wakeLock;
    @Override public IBinder onBind(Intent intent) { return null; }
    @Override public int onStartCommand(Intent intent,int flags,int startId) {
        if(working) return START_NOT_STICKY;
        String action=intent==null?null:intent.getAction();
        synchronized(HostState.LOCK) {
            boolean checking=CHECK.equals(action);
            if(BuildConfig.CORE_HOST_API!=1||action==null||HostState.operation!=null||HostState.phase==HostState.Phase.STARTING||HostState.phase==HostState.Phase.STOPPING||(!checking&&!HostState.idle())) { stopSelf();return START_NOT_STICKY; }
            HostState.operation=checking?"Проверяем обновления ядра…":"Обновляем ядро…";HostState.coreUpdateMessage=HostState.operation;HostState.coreDownloadBytes=HostState.coreDownloadTotal=0;HostState.coreUpdateRevision++;working=true;
        }
        try {
            notification();wakeLock=((PowerManager)getSystemService(POWER_SERVICE)).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK,"mcsa:core-update");wakeLock.setReferenceCounted(false);wakeLock.acquire(60L*60*1000);
            new Thread(()->run(action),"mcsa-core-update").start();
        } catch(Exception error) { finish("Не удалось начать обновление ядра."); }
        return START_NOT_STICKY;
    }
    private void notification() {
        NotificationManager manager=(NotificationManager)getSystemService(NOTIFICATION_SERVICE);
        if(Build.VERSION.SDK_INT>=26) manager.createNotificationChannel(new NotificationChannel("core-updates","Обновления ядра",NotificationManager.IMPORTANCE_LOW));
        Notification.Builder builder=Build.VERSION.SDK_INT>=26?new Notification.Builder(this,"core-updates"):new Notification.Builder(this);
        PendingIntent open=PendingIntent.getActivity(this,3,new Intent(this,MainActivity.class),PendingIntent.FLAG_UPDATE_CURRENT|PendingIntent.FLAG_IMMUTABLE);
        Notification notification=builder.setSmallIcon(R.drawable.project_logo_monochrome).setContentTitle("Обновление ядра MCSheriffAnya").setContentText("Проверка, загрузка или сохранение копии…").setContentIntent(open).setOngoing(true).build();
        if(Build.VERSION.SDK_INT>=34) startForeground(3,notification,ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE);else startForeground(3,notification);
    }
    private void run(String action) {
        String result;
        try {
            CoreUpdateStore store=new CoreUpdateStore(this);
            if(CHECK.equals(action)) {
                CoreManifest manifest=store.manifest(CoreDownload.manifest(BuildConfig.CORE_MANIFEST_URL));boolean newer=manifest.revision>store.revision();
                synchronized(HostState.LOCK) { HostState.coreAvailable=newer?manifest:null;HostState.coreVersion=store.version(); }
                result=newer?"Доступна новая версия ядра: "+manifest.version+"\n"+manifest.notes:"У тебя установлена актуальная версия ядра.";
            } else if(INSTALL.equals(action)) {
                CoreManifest manifest;synchronized(HostState.LOCK) { manifest=HostState.coreAvailable; }
                if(manifest==null) throw new IOException("Сначала проверь наличие обновлений");
                CoreUpdateStore.Core candidate=store.stage(manifest,(bytes,total)->{
                    synchronized(HostState.LOCK) { HostState.coreDownloadBytes=bytes;HostState.coreDownloadTotal=total;HostState.coreUpdateMessage="Скачиваем ядро: "+android.text.format.Formatter.formatFileSize(this,bytes)+" / "+android.text.format.Formatter.formatFileSize(this,total);HostState.coreUpdateRevision++; }
                });
                synchronized(HostState.LOCK) { HostState.coreUpdateMessage="Сохраняем резервную копию мира перед обновлением…";HostState.coreUpdateRevision++; }
                BackupStore backup=new BackupStore(getFilesDir());backup.recover();File copy=backup.create();store.activate(candidate,copy.getName());
                synchronized(HostState.LOCK) { HostState.coreAvailable=null;HostState.coreVersion=candidate.version;HostState.backupRevision++; }
                result="Новое ядро подготовлено. Запусти сервер — он будет работать на "+candidate.version+".\nКопия прежних данных: "+copy.getName();
            } else if(ROLLBACK.equals(action)) {
                BackupStore backup=new BackupStore(getFilesDir());backup.recover();File copy=backup.create();store.rollback();
                synchronized(HostState.LOCK) { HostState.coreVersion=store.version();HostState.backupRevision++; }
                result="Выбрана предыдущая версия ядра. Можно запускать сервер.\nКопия текущего мира: "+copy.getName();
            } else throw new IOException("Неизвестная операция обновления");
        } catch(Exception error) {
            String message=error.getMessage();result=message!=null&&!message.isEmpty()&&message.charAt(0)>127?message:"Не удалось связаться с сервером обновлений. Проверь интернет и попробуй ещё раз.";
        }
        String message=result;new Handler(Looper.getMainLooper()).post(()->finish(message));
    }
    private void finish(String result) {
        synchronized(HostState.LOCK) { HostState.coreUpdateMessage=result;HostState.coreUpdateRevision++; }
        finished=true;if(wakeLock!=null&&wakeLock.isHeld()) wakeLock.release();wakeLock=null;
        if(destroyed) releaseOperation();else { stopForeground(true);stopSelf(); }
    }
    private void releaseOperation() { synchronized(HostState.LOCK) { HostState.operation=null;HostState.coreUpdateRevision++; } }
    @Override public void onDestroy() { destroyed=true;if(finished) releaseOperation();super.onDestroy(); }
}
