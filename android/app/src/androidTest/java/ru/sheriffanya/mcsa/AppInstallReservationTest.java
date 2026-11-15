package ru.sheriffanya.mcsa;

import android.content.Context;
import androidx.test.core.app.ApplicationProvider;
import org.json.JSONObject;
import org.junit.Test;
import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicBoolean;
import static org.junit.Assert.*;

/** Проверяет настоящую блокировку Store/HostState и durable marker, без запуска сервера. */
public class AppInstallReservationTest {
    /** abandonSession возвращается до фактического исчезновения SessionInfo. */
    private void awaitSessionGoneThenRelease(Context app,int id) throws Exception {
        android.content.pm.PackageInstaller installer=app.getPackageManager().getPackageInstaller();
        long deadline=android.os.SystemClock.elapsedRealtime()+15000;
        while(installer.getSessionInfo(id)!=null) {
            assertFalse("Сервер остаётся заблокирован, пока системная сессия существует",HostState.idle());
            assertTrue("APK остаётся зарезервирован до исчезновения сессии",AppUpdateStore.busy);
            assertEquals(AppUpdateStore.INSTALL_OPERATION,HostState.operation);
            if(android.os.SystemClock.elapsedRealtime()>=deadline) fail("Android не удалил отменённую сессию "+id+" за 15 секунд");
            Thread.sleep(50);
        }
        assertNull(installer.getSessionInfo(id));AppInstallSession.reconcile(app);
        assertTrue("Блокировка снята только после исчезновения сессии",HostState.idle());assertFalse(AppUpdateStore.busy);assertFalse(AppUpdateStore.hasInstallation());
    }
    private void cleanupInstallation(Context app) throws Exception {
        AppUpdateStore.Installation ticket=AppUpdateStore.currentInstallation();if(ticket==null) return;
        AppInstallSession.cancel(app,ticket,"test cleanup");awaitSessionGoneThenRelease(app,ticket.sessionId);
    }
    private AppUpdateManifest description() throws Exception {
        JSONObject file=new JSONObject().put("bytes",1000000).put("sha256","0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef").put("url","https://github.com/AnyaSheriff/MCSheriffAnya/releases/download/test/test.apk");
        JSONObject json=new JSONObject().put("format",1).put("package","ru.sheriffanya.mcsa").put("version_code",BuildConfig.VERSION_CODE+1).put("version_name","test").put("apks",new JSONObject().put(BuildConfig.RUNTIME_ABI,file));
        return AppUpdateManifest.parse(json.toString().getBytes(StandardCharsets.UTF_8),BuildConfig.RUNTIME_ABI,BuildConfig.VERSION_CODE);
    }
    @Test public void reserveBlocksOtherWorkAndCancelOrErrorReleases() throws Exception {
        Context app=ApplicationProvider.getApplicationContext();assertTrue(HostState.idle());assertFalse(AppUpdateStore.busy);
        File previous=AppUpdateStore.ready;AppUpdateManifest oldReady=AppUpdateStore.readyManifest,oldAvailable=AppUpdateStore.available;
        try {
            AppUpdateStore.ready=new File(new File(app.getCacheDir(),"app-updates"),UUID.randomUUID()+".apk");AppUpdateStore.readyManifest=description();AppUpdateStore.available=description();
            AppUpdateStore.Installation ticket=AppUpdateStore.beginInstallation(app);
            assertFalse(HostState.idle());assertEquals(AppUpdateStore.INSTALL_OPERATION,HostState.operation);assertTrue(AppUpdateStore.busy);assertFalse(AppUpdateStore.canInstall());
            AtomicBoolean unexpectedDownload=new AtomicBoolean();File retained=AppUpdateStore.ready;
            AppUpdateStore.download(app,()->unexpectedDownload.set(true));AppUpdateStore.check(app,()->unexpectedDownload.set(true));
            assertFalse(unexpectedDownload.get());assertSame(retained,AppUpdateStore.ready);
            try { AppUpdateStore.beginInstallation(app);fail("double reservation"); }catch(IOException expected) { assertNotNull(expected.getMessage()); }
            AppUpdateStore.installerLaunched(ticket);assertFalse(AppUpdateStore.canInstall());
            AppUpdateStore.finishInstallation(app,ticket,"cancelled");assertTrue(HostState.idle());assertFalse(AppUpdateStore.busy);assertFalse(AppUpdateStore.hasInstallation());
            ticket=AppUpdateStore.beginInstallation(app);AppUpdateStore.finishInstallation(app,ticket,"missing installer");assertTrue(HostState.idle());assertFalse(AppUpdateStore.busy);
            assertEquals(0,app.getSharedPreferences("app-install-pending",Context.MODE_PRIVATE).getLong("code",0));
            ticket=AppUpdateStore.beginInstallation(app);
            try { AppInstallSession.prepare(app,ticket);fail("missing APK accepted"); }catch(IOException expected) { assertNotNull(expected.getMessage()); }
            awaitSessionGoneThenRelease(app,ticket.sessionId);assertTrue(app.getPackageManager().getPackageInstaller().getMySessions().isEmpty());
        } finally { cleanupInstallation(app);AppUpdateStore.ready=previous;AppUpdateStore.readyManifest=oldReady;AppUpdateStore.available=oldAvailable; }
    }
    @Test public void actualPackageInstallerSessionRetainsExactApkAndCancelDestroysIt() throws Exception {
        Context app=ApplicationProvider.getApplicationContext();assertTrue(HostState.idle());
        String path=androidx.test.platform.app.InstrumentationRegistry.getArguments().getString("appUpgradeApk");
        org.junit.Assume.assumeTrue("Нужен appUpgradeApk",path!=null);
        File fixture=new File(path);assertTrue(fixture.isFile());
        File previous=AppUpdateStore.ready;AppUpdateManifest oldReady=AppUpdateStore.readyManifest;
        File cached=new File(new File(app.getCacheDir(),"app-updates"),UUID.randomUUID()+".apk");assertTrue(cached.getParentFile().isDirectory()||cached.getParentFile().mkdirs());
        try {
            try(java.io.InputStream in=new java.io.FileInputStream(fixture);java.io.OutputStream out=new java.io.FileOutputStream(cached)) { byte[] b=new byte[65536];int n;while((n=in.read(b))!=-1) out.write(b,0,n); }
            JSONObject file=new JSONObject().put("bytes",cached.length()).put("sha256",CoreDownload.sha256(cached)).put("url","https://github.com/AnyaSheriff/MCSheriffAnya/releases/download/test/test.apk");
            JSONObject json=new JSONObject().put("format",1).put("package","ru.sheriffanya.mcsa").put("version_code",BuildConfig.VERSION_CODE+1).put("version_name",BuildConfig.VERSION_NAME).put("apks",new JSONObject().put(BuildConfig.RUNTIME_ABI,file));
            AppUpdateStore.ready=cached;AppUpdateStore.readyManifest=AppUpdateManifest.parse(json.toString().getBytes(StandardCharsets.UTF_8),BuildConfig.RUNTIME_ABI,BuildConfig.VERSION_CODE);
            AppUpdateStore.verify(app,cached,AppUpdateStore.readyManifest);
            AppUpdateStore.Installation ticket=AppUpdateStore.beginInstallation(app);
            int id=AppInstallSession.prepare(app,ticket);assertTrue(id>=0);assertFalse(HostState.idle());assertTrue(AppUpdateStore.busy);
            android.content.pm.PackageInstaller installer=app.getPackageManager().getPackageInstaller();assertNotNull(installer.getSessionInfo(id));
            try(android.content.pm.PackageInstaller.Session session=installer.openSession(id);java.io.InputStream in=session.openRead("base.apk")) {
                java.security.MessageDigest digest=java.security.MessageDigest.getInstance("SHA-256");byte[] b=new byte[65536];int n;long length=0;
                while((n=in.read(b))!=-1) { length+=n;digest.update(b,0,n); }assertEquals(cached.length(),length);
                StringBuilder hex=new StringBuilder();for(byte v:digest.digest()) hex.append(String.format(java.util.Locale.ROOT,"%02x",v&255));assertEquals(CoreDownload.sha256(cached),hex.toString());
            }
            AppUpdateStore.recoverInstallation(app);AppInstallSession.reconcile(app);assertFalse(HostState.idle());assertTrue(AppUpdateStore.busy);assertEquals(id,AppUpdateStore.currentInstallation().sessionId);
            AppInstallSession.cancel(app,AppUpdateStore.currentInstallation(),"cancelled");awaitSessionGoneThenRelease(app,id);
            // Пропавшая сессия после гибели процесса также снимает только нашу блокировку.
            AppUpdateStore.ready=cached;AppUpdateStore.Installation missing=AppUpdateStore.beginInstallation(app);int absent=AppInstallSession.prepare(app,missing);installer.abandonSession(absent);
            AppUpdateStore.recoverInstallation(app);awaitSessionGoneThenRelease(app,absent);
        } finally { cleanupInstallation(app);cached.delete();AppUpdateStore.ready=previous;AppUpdateStore.readyManifest=oldReady; }
    }
    @Test public void processRecoveryRestoresReservationAndStaleTicketCannotReleaseIt() throws Exception {
        Context app=ApplicationProvider.getApplicationContext();assertTrue(HostState.idle());
        File previous=AppUpdateStore.ready;AppUpdateManifest oldReady=AppUpdateStore.readyManifest;
        try {
            AppUpdateStore.ready=new File(new File(app.getCacheDir(),"app-updates"),UUID.randomUUID()+".apk");AppUpdateStore.readyManifest=description();
            AppUpdateStore.Installation stale=AppUpdateStore.beginInstallation(app);AppUpdateStore.installerLaunched(stale);
            synchronized(HostState.LOCK) { HostState.operation=null; }
            AppUpdateStore.recoverInstallation(app);assertFalse(HostState.idle());assertTrue(AppUpdateStore.busy);
            AppUpdateStore.finishInstallation(app,stale,"stale callback");assertFalse(HostState.idle());assertTrue(AppUpdateStore.hasInstallation());
            AppUpdateStore.finishInstallation(app,null,"cancelled");assertTrue(HostState.idle());
        } finally { AppUpdateStore.finishInstallation(app,null,"test cleanup");AppUpdateStore.ready=previous;AppUpdateStore.readyManifest=oldReady; }
    }
}
