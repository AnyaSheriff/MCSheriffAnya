package ru.sheriffanya.mcsa;

import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageInstaller;
import android.os.Build;
import java.io.*;

/** Одна системная сессия: блокировка снимается только после её завершения/уничтожения. */
final class AppInstallSession {
    private static final Object SESSION_LOCK=new Object();
    private static Intent confirmation;
    private static boolean confirmationShown;
    static synchronized void clearConfirmation() { confirmation=null;confirmationShown=false; }
    static synchronized boolean hasConfirmation() { return confirmation!=null; }
    static synchronized Intent confirmation(boolean onlyUnshown) {
        if(confirmation==null||(onlyUnshown&&confirmationShown)) return null;
        confirmationShown=true;return new Intent(confirmation);
    }
    static void commit(Context context,AppUpdateStore.Installation ticket) throws Exception {
        synchronized(SESSION_LOCK) {
            int id=prepareLocked(context,ticket);
            PackageInstaller installer=context.getPackageManager().getPackageInstaller();
            try(PackageInstaller.Session session=installer.openSession(id)) {
                Intent callback=new Intent(context,AppInstallReceiver.class).setAction("ru.sheriffanya.mcsa.APP_INSTALL_RESULT").setPackage(context.getPackageName()).setData(android.net.Uri.parse("mcsa-app-install:"+id));
                int flags=PendingIntent.FLAG_UPDATE_CURRENT;if(Build.VERSION.SDK_INT>=31) flags|=PendingIntent.FLAG_MUTABLE;
                PendingIntent result=PendingIntent.getBroadcast(context,id,callback,flags);
                AppUpdateStore.installerLaunched(ticket);session.commit(result.getIntentSender());
                AppUpdateStore.message="Подтверди установку приложения в Android.";
            } catch(Exception error) { cancelLocked(context,ticket,"Установка не началась. Попробуй ещё раз.");throw error; }
        }
    }
    /** Отдельный этап позволяет проверить настоящую сессию без установки в CI. */
    static int prepare(Context context,AppUpdateStore.Installation ticket) throws Exception {
        synchronized(SESSION_LOCK) { return prepareLocked(context,ticket); }
    }
    private static int prepareLocked(Context context,AppUpdateStore.Installation ticket) throws Exception {
        if(AppUpdateStore.currentInstallation()!=ticket) throw new IOException("Ожидание установки изменилось");
        PackageInstaller installer=context.getPackageManager().getPackageInstaller();
        if(!installer.getMySessions().isEmpty()) throw new IOException("Уже существует незавершённая установочная сессия");
        PackageInstaller.SessionParams params=new PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL);
        params.setAppPackageName(context.getPackageName());params.setSize(ticket.manifest.bytes);
        if(Build.VERSION.SDK_INT>=31) params.setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_REQUIRED);
        int id=installer.createSession(params);
        try {
            AppUpdateStore.sessionCreated(context,ticket,id);
            try(PackageInstaller.Session session=installer.openSession(id)) {
                try(InputStream in=new FileInputStream(ticket.apk);OutputStream out=session.openWrite("base.apk",0,ticket.manifest.bytes)) {
                    byte[] buffer=new byte[65536];int n;long total=0;java.security.MessageDigest digest=java.security.MessageDigest.getInstance("SHA-256");
                    while((n=in.read(buffer))!=-1) { total+=n;if(total>ticket.manifest.bytes) throw new IOException("Размер APK изменился");out.write(buffer,0,n);digest.update(buffer,0,n); }
                    if(total!=ticket.manifest.bytes) throw new IOException("APK загрузился не полностью");
                    StringBuilder hex=new StringBuilder();for(byte b:digest.digest()) hex.append(String.format(java.util.Locale.ROOT,"%02x",b&255));
                    if(!hex.toString().equals(ticket.manifest.sha256)) throw new IOException("APK изменился во время подготовки установки");session.fsync(out);
                }
            }
            return id;
        } catch(Exception error) { try { installer.abandonSession(id); }catch(RuntimeException ignored) { }cancelLocked(context,ticket,"Подготовка установки не завершилась. Попробуй ещё раз.");throw error; }
    }
    /** Вызывается до запуска сервера, в том числе после гибели процесса/перезагрузки. */
    static void reconcile(Context context) {
        AppUpdateStore.Installation ticket=AppUpdateStore.currentInstallation();PackageInstaller installer=context.getPackageManager().getPackageInstaller();
        if(ticket==null) {
            // Только собственные сессии: процесс мог погибнуть между createSession и записью id.
            for(PackageInstaller.SessionInfo orphan:installer.getMySessions()) try { installer.abandonSession(orphan.getSessionId()); }catch(RuntimeException ignored) { }
            return;
        }
        if(ticket.sessionId<0) { cancel(context,ticket,"Подготовка установки была прервана. Можно попробовать снова.");return; }
        if(installer.getSessionInfo(ticket.sessionId)==null) AppUpdateStore.finishInstallation(context,ticket,"Установка завершена или отменена. Можно проверить обновления снова.");
    }
    static void cancel(Context context,AppUpdateStore.Installation ticket,String message) {
        synchronized(SESSION_LOCK) { cancelLocked(context,ticket,message); }
    }
    private static void cancelLocked(Context context,AppUpdateStore.Installation ticket,String message) {
        if(ticket==null||AppUpdateStore.currentInstallation()!=ticket) return;
        PackageInstaller installer=context.getPackageManager().getPackageInstaller();
        try {
            if(ticket.sessionId>=0) {
                if(installer.getSessionInfo(ticket.sessionId)!=null) installer.abandonSession(ticket.sessionId);
                if(installer.getSessionInfo(ticket.sessionId)!=null) { AppUpdateStore.message="Android завершает отмену установки. Подожди.";return; }
            } else {
                for(PackageInstaller.SessionInfo orphan:installer.getMySessions()) installer.abandonSession(orphan.getSessionId());
                if(!installer.getMySessions().isEmpty()) { AppUpdateStore.message="Android завершает отмену установки. Подожди.";return; }
            }
            AppUpdateStore.finishInstallation(context,ticket,message);
        } catch(RuntimeException active) { AppUpdateStore.message="Android ещё выполняет установку. Дождись результата или попробуй отмену снова."; }
    }
    static void result(Context context,Intent intent) {
        synchronized(SESSION_LOCK) { resultLocked(context,intent); }
    }
    private static void resultLocked(Context context,Intent intent) {
        AppUpdateStore.Installation ticket=AppUpdateStore.currentInstallation();
        int id=intent.getIntExtra(PackageInstaller.EXTRA_SESSION_ID,-1);
        if(ticket==null||id!=ticket.sessionId||!intent.hasExtra(PackageInstaller.EXTRA_STATUS)) return;
        int status=intent.getIntExtra(PackageInstaller.EXTRA_STATUS,PackageInstaller.STATUS_FAILURE);
        if(status==PackageInstaller.STATUS_PENDING_USER_ACTION) {
            @SuppressWarnings("deprecation") Intent request=intent.getParcelableExtra(Intent.EXTRA_INTENT);
            if(request==null) { cancel(context,ticket,"Android не предоставил подтверждение установки.");return; }
            synchronized(AppInstallSession.class) { confirmation=new Intent(request);confirmationShown=false; }
            AppUpdateStore.message="Подтверди установку приложения в Android.";return;
        }
        if(status==PackageInstaller.STATUS_SUCCESS) AppUpdateStore.finishInstallation(context,ticket,"Приложение обновлено.");
        else cancel(context,ticket,"Установка отменена или не выполнена. Можно попробовать снова.");
    }
}
