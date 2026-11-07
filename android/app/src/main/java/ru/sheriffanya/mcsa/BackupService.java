package ru.sheriffanya.mcsa;

import android.app.*;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.net.Uri;
import android.os.*;
import java.io.*;

/** Копирование продолжается при закрытом экране; URI даёт сам системный выбор файла. */
public final class BackupService extends Service {
    static final String CREATE="mcsa.backup.CREATE",EXPORT="mcsa.backup.EXPORT",IMPORT="mcsa.backup.IMPORT",RESTORE="mcsa.backup.RESTORE";
    private boolean working,finished,destroyed;
    private PowerManager.WakeLock wakeLock;
    @Override public IBinder onBind(Intent intent) { return null; }
    @Override public int onStartCommand(Intent intent,int flags,int startId) {
        if(working) return START_NOT_STICKY;
        synchronized(HostState.LOCK) {
            if(intent==null||!HostState.idle()) { stopSelf();return START_NOT_STICKY; }
            HostState.operation="Работа с резервной копией…";HostState.backupMessage=HostState.operation;HostState.backupRevision++;
            working=true;
        }
        try {
            notification();
            wakeLock=((PowerManager)getSystemService(POWER_SERVICE)).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK,"mcsa:backup");wakeLock.setReferenceCounted(false);wakeLock.acquire(60L*60*1000);
            new Thread(()->run(intent),"mcsa-backup").start();
        } catch(Exception error) { finish("Не удалось начать копирование: "+error.getMessage()); }
        return START_NOT_STICKY;
    }
    private void notification() {
        NotificationManager manager=(NotificationManager)getSystemService(NOTIFICATION_SERVICE);
        if(Build.VERSION.SDK_INT>=26) manager.createNotificationChannel(new NotificationChannel("backups","Резервные копии",NotificationManager.IMPORTANCE_LOW));
        Notification.Builder builder=Build.VERSION.SDK_INT>=26?new Notification.Builder(this,"backups"):new Notification.Builder(this);
        PendingIntent open=PendingIntent.getActivity(this,2,new Intent(this,MainActivity.class),PendingIntent.FLAG_UPDATE_CURRENT|PendingIntent.FLAG_IMMUTABLE);
        Notification notification=builder.setSmallIcon(R.drawable.project_logo_monochrome).setContentTitle("Резервная копия MCSheriffAnya").setContentText("Подготовка или восстановление данных…").setContentIntent(open).setOngoing(true).build();
        if(Build.VERSION.SDK_INT>=34) startForeground(2,notification,ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE);else startForeground(2,notification);
    }
    private void run(Intent intent) {
        String result;
        try {
            BackupStore store=new BackupStore(getFilesDir());store.recover();
            String action=intent.getAction();
            if(CREATE.equals(action)) result="Копия создана: "+store.create().getName();
            else if(EXPORT.equals(action)) {
                File file=store.file(intent.getStringExtra("name"));Uri uri=uri(intent);
                try(InputStream in=new FileInputStream(file);OutputStream out=getContentResolver().openOutputStream(uri,"wt")) {
                    if(out==null) throw new IOException("Выбранный файл недоступен.");byte[] buffer=new byte[32768];int n;while((n=in.read(buffer))!=-1) out.write(buffer,0,n);out.flush();
                }
                result="Копия сохранена в выбранный файл телефона.";
            } else if(IMPORT.equals(action)||RESTORE.equals(action)) {
                String previous;
                try(InputStream in=IMPORT.equals(action)?getContentResolver().openInputStream(uri(intent)):new FileInputStream(store.file(intent.getStringExtra("name")))) {
                    if(in==null) throw new IOException("Выбранный файл недоступен.");previous=store.restore(in);
                }
                result="Копия восстановлена. Сервер можно запускать."+(previous==null?"":"\nПрежние данные: "+previous);
            } else throw new IOException("Неизвестная операция с копией.");
        } catch(Exception error) { result="Операция не завершена: "+error.getMessage(); }
        String message=result;new Handler(Looper.getMainLooper()).post(()->finish(message));
    }
    private Uri uri(Intent intent) throws IOException {
        Uri uri=intent.getData();if(uri==null||!"content".equals(uri.getScheme())) throw new IOException("Выбери файл через системное меню.");return uri;
    }
    private void finish(String result) {
        synchronized(HostState.LOCK) { HostState.backupMessage=result; }
        finished=true;if(wakeLock!=null&&wakeLock.isHeld()) wakeLock.release();wakeLock=null;
        if(destroyed) releaseOperation();else { stopForeground(true);stopSelf(); }
    }
    private void releaseOperation() {
        synchronized(HostState.LOCK) { HostState.operation=null;HostState.backupRevision++; }
    }
    @Override public void onDestroy() {
        // Блокировка снимается после worker и ухода service: новый запуск не попадёт в старый stopSelf.
        destroyed=true;if(finished) releaseOperation();super.onDestroy();
    }
}
