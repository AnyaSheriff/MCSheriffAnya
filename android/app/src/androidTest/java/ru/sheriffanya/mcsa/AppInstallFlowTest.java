package ru.sheriffanya.mcsa;

import android.content.Context;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.os.Build;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import androidx.test.core.app.ActivityScenario;
import androidx.test.platform.app.InstrumentationRegistry;
import org.json.JSONObject;
import org.junit.Test;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import static org.junit.Assert.*;

/** Настоящее обновление собственного APK. Системное подтверждение выполняет внешний CI driver. */
public class AppInstallFlowTest {
    private Context context() { return InstrumentationRegistry.getInstrumentation().getTargetContext(); }
    private long code(PackageInfo info) { return Build.VERSION.SDK_INT>=28?info.getLongVersionCode():info.versionCode; }
    private JSONObject installed() throws Exception {
        PackageInfo info=context().getPackageManager().getPackageInfo(context().getPackageName(),PackageManager.GET_SIGNATURES);
        JSONObject result=new JSONObject().put("code",code(info)).put("version",info.versionName).put("first_install_time",info.firstInstallTime);
        org.json.JSONArray signatures=new org.json.JSONArray();for(android.content.pm.Signature sig:info.signatures) signatures.put(android.util.Base64.encodeToString(sig.toByteArray(),android.util.Base64.NO_WRAP));
        return result.put("signatures",signatures);
    }
    private void write(String name,JSONObject data) throws Exception {
        try(FileOutputStream out=new FileOutputStream(new File(context().getFilesDir(),name))) { out.write((data.toString(2)+"\n").getBytes(StandardCharsets.UTF_8));out.getFD().sync(); }
    }
    private void hashes(File root,String relative,JSONObject result) throws Exception {
        File file=new File(root,relative);if(!file.exists()) return;
        if(file.isDirectory()) { String[] children=file.list();assertNotNull(children);Arrays.sort(children);for(String child:children) hashes(root,relative+"/"+child,result); }
        else result.put(relative,CoreDownload.sha256(file));
    }
    private JSONObject retained() throws Exception {
        File root=context().getFilesDir();JSONObject result=new JSONObject();
        for(String path:new String[]{"server","backups","core-updates"}) hashes(root,path,result);
        assertTrue("baseline world marker exists",result.has("server/world/apk-persistence-marker"));
        assertTrue("real config exists",result.has("server/config/mcsa.properties"));
        File[] backups=new File(root,"backups").listFiles((dir,name)->name.endsWith(".zip"));assertNotNull(backups);assertTrue("real ZIP backups retained",backups.length>0);
        assertTrue("selected core pointer exists",result.has("core-updates/active"));
        return result;
    }
    private boolean textClick(View view,String prefix) {
        if(view instanceof Button&&((Button)view).getText().toString().startsWith(prefix)&&view.isEnabled()) { view.performClick();return true; }
        if(view instanceof ViewGroup) { ViewGroup group=(ViewGroup)view;for(int i=0;i<group.getChildCount();i++) if(textClick(group.getChildAt(i),prefix)) return true; }
        return false;
    }
    private AppUpdateManifest stageCandidate() throws Exception {
        assertTrue(HostState.idle());Context app=context();PackageManager pm=app.getPackageManager();
        File input=new File(InstrumentationRegistry.getArguments().getString("appUpgradeApk","/data/local/tmp/mcsa-upgrade.apk"));
        PackageInfo candidate=pm.getPackageArchiveInfo(input.getAbsolutePath(),0);assertNotNull(candidate);long target=code(candidate);
        assertEquals(7,target);assertEquals(6,installed().getLong("code"));
        File dir=new File(app.getCacheDir(),"app-updates");assertTrue(dir.isDirectory()||dir.mkdirs());File apk=new File(dir,java.util.UUID.randomUUID()+".apk");
        try(InputStream in=new FileInputStream(input);FileOutputStream out=new FileOutputStream(apk)) { byte[] buffer=new byte[65536];int n;while((n=in.read(buffer))!=-1) out.write(buffer,0,n);out.getFD().sync(); }
        JSONObject item=new JSONObject().put("bytes",apk.length()).put("sha256",CoreDownload.sha256(apk)).put("url","https://github.com/AnyaSheriff/MCSheriffAnya/releases/download/test/test.apk");
        JSONObject json=new JSONObject().put("format",1).put("package",app.getPackageName()).put("version_code",target).put("version_name",candidate.versionName).put("apks",new JSONObject().put(BuildConfig.RUNTIME_ABI,item));
        AppUpdateManifest manifest=AppUpdateManifest.parse(json.toString().getBytes(StandardCharsets.UTF_8),BuildConfig.RUNTIME_ABI,installed().getLong("code"));
        AppUpdateStore.verify(app,apk,manifest);AppUpdateStore.ready=apk;AppUpdateStore.readyManifest=manifest;
        return manifest;
    }
    private void sameFiles(JSONObject expected,JSONObject actual) throws Exception {
        assertEquals(expected.length(),actual.length());java.util.Iterator<String> keys=expected.keys();
        while(keys.hasNext()) { String key=keys.next();assertEquals(key,expected.getString(key),actual.getString(key)); }
    }
    @Test public void prepareInterruptedSession() throws Exception {
        AppUpdateManifest manifest=stageCandidate();Context app=context();
        AppUpdateStore.Installation ticket=AppUpdateStore.beginInstallation(app);
        int id=AppInstallSession.prepare(app,ticket);assertEquals(id,ticket.sessionId);
        assertNotNull(app.getPackageManager().getPackageInstaller().getSessionInfo(id));
        assertFalse(HostState.idle());assertTrue(AppUpdateStore.busy);
        write("install-interruption-before.json",new JSONObject().put("api",Build.VERSION.SDK_INT).put("pid",android.os.Process.myPid()).put("session_id",id).put("apk_name",ticket.apk.getName()).put("apk_sha256",CoreDownload.sha256(ticket.apk)).put("target",manifest.code).put("retained",retained()));
        // Без commit: внешний скрипт действительно убивает процесс, Application восстанавливает marker.
    }
    @Test public void verifyInterruptedSessionAndCancel() throws Exception {
        Context app=context();JSONObject before=new JSONObject(ServerSettings.read(new File(app.getFilesDir(),"install-interruption-before.json")));
        assertEquals(6,installed().getLong("code"));assertTrue("new process after force-stop",before.getInt("pid")!=android.os.Process.myPid());
        int id=before.getInt("session_id");boolean exists=app.getPackageManager().getPackageInstaller().getSessionInfo(id)!=null;
        sameFiles(before.getJSONObject("retained"),retained());
        if(exists) {
            AppUpdateStore.Installation ticket=AppUpdateStore.currentInstallation();assertNotNull("Application recovered ticket",ticket);
            assertEquals(id,ticket.sessionId);assertEquals(before.getString("apk_name"),ticket.apk.getName());
            assertEquals(before.getString("apk_sha256"),CoreDownload.sha256(ticket.apk));
            assertEquals(before.getLong("target"),AppUpdateStore.pending(app).getLong("code",0));
            assertEquals(id,AppUpdateStore.pending(app).getInt("session",-1));
            assertFalse(HostState.idle());assertTrue(AppUpdateStore.busy);assertEquals(AppUpdateStore.INSTALL_OPERATION,HostState.operation);assertFalse(AppUpdateStore.canInstall());
            AppInstallSession.cancel(app,ticket,"CI: interrupted session cancelled");
        } else {
            assertTrue("Application reconciled removed session",HostState.idle());assertFalse(AppUpdateStore.hasInstallation());assertFalse(AppUpdateStore.busy);assertEquals(0,AppUpdateStore.pending(app).getLong("code",0));
        }
        long end=System.currentTimeMillis()+30000;
        while(System.currentTimeMillis()<end) {
            AppInstallSession.reconcile(app);
            if(app.getPackageManager().getPackageInstaller().getSessionInfo(id)==null&&HostState.idle()&&!AppUpdateStore.hasInstallation()) break;
            Thread.sleep(100);
        }
        assertNull(app.getPackageManager().getPackageInstaller().getSessionInfo(id));assertTrue(HostState.idle());assertFalse(AppUpdateStore.hasInstallation());assertFalse(AppUpdateStore.busy);
        assertEquals(0,AppUpdateStore.pending(app).getLong("code",0));assertTrue(app.getPackageManager().getPackageInstaller().getMySessions().isEmpty());
        sameFiles(before.getJSONObject("retained"),retained());
        write("install-interruption-after.json",new JSONObject().put("status","passed").put("actual_retained_or_system_removed",exists?"session_retained_and_cancelled":"system_removed_session_recovered_idle").put("session_id",id).put("pid",android.os.Process.myPid()).put("data_retained",true).put("cleanup",true).put("host_idle",true));
    }
    @Test public void prepareSystemInstall() throws Exception {
        AppUpdateManifest manifest=stageCandidate();Context app=context();PackageManager pm=app.getPackageManager();long target=manifest.code;
        JSONObject before=new JSONObject().put("api",Build.VERSION.SDK_INT).put("target",target).put("installed",installed()).put("candidate_sha256",manifest.sha256).put("retained",retained());
        write("install-flow-before.json",before);
        // Не закрываем ActivityScenario: установщик остаётся открыт после окончания instrumentation.
        ActivityScenario<MainActivity> activity=ActivityScenario.launch(MainActivity.class);
        activity.onActivity(a->{ a.findViewById(R.id.tab_settings).performClick();assertTrue(textClick(a.getWindow().getDecorView(),"Обновления"));assertTrue(textClick(a.getWindow().getDecorView(),"Приложение Android")); });
        long cacheDeadline=System.currentTimeMillis()+30000;java.util.concurrent.atomic.AtomicBoolean installEnabled=new java.util.concurrent.atomic.AtomicBoolean();
        while(System.currentTimeMillis()<cacheDeadline) { activity.onActivity(a->{View v=a.findViewById(R.id.app_install_update);installEnabled.set(v!=null&&v.isEnabled());});if(installEnabled.get()) break;Thread.sleep(100); }assertTrue("install button enabled",installEnabled.get());
        activity.onActivity(a->{View install=a.findViewById(R.id.app_install_update);assertTrue(install.isEnabled());install.performClick(); });
        long end=System.currentTimeMillis()+30000;
        while(System.currentTimeMillis()<end) { AppUpdateStore.Installation ticket=AppUpdateStore.currentInstallation();if(ticket!=null&&ticket.sessionId>=0&&AppInstallSession.hasConfirmation()) break;Thread.sleep(100); }
        AppUpdateStore.Installation ticket=AppUpdateStore.currentInstallation();assertNotNull(ticket);assertTrue(ticket.sessionId>=0);assertTrue(AppInstallSession.hasConfirmation());
        assertFalse(HostState.idle());assertFalse(AppUpdateStore.canInstall());
        before.put("session_id",ticket.sessionId).put("session_present",pm.getPackageInstaller().getSessionInfo(ticket.sessionId)!=null).put("pending_code",AppUpdateStore.pending(app).getLong("code",0));
        assertTrue(before.getBoolean("session_present"));assertEquals(target,before.getLong("pending_code"));write("install-flow-before.json",before);
        // STATUS_PENDING_USER_ACTION содержит Intent, но UI ещё может ждать refresh Activity.
        // Не заканчиваем instrumentation, пока настоящий системный установщик не виден.
        androidx.test.uiautomator.UiDevice device=androidx.test.uiautomator.UiDevice.getInstance(InstrumentationRegistry.getInstrumentation());
        long uiDeadline=System.currentTimeMillis()+30000;String installerPackage=null;
        while(System.currentTimeMillis()<uiDeadline) {
            String visible=device.getCurrentPackageName();
            if("com.android.packageinstaller".equals(visible)||"com.google.android.packageinstaller".equals(visible)||"com.android.permissioncontroller".equals(visible)||"com.google.android.permissioncontroller".equals(visible)) {
                android.content.pm.ApplicationInfo installer=pm.getApplicationInfo(visible,0);
                assertTrue("trusted system installer",(installer.flags&android.content.pm.ApplicationInfo.FLAG_SYSTEM)!=0);installerPackage=visible;break;
            }
            Thread.sleep(100);
        }
        assertNotNull("actual system installer foreground before instrumentation cleanup",installerPackage);
        before.put("installer_foreground_before_finish",installerPackage);write("install-flow-before.json",before);

    }
    @Test public void postInstallRetainsDataAndReleasesReservation() throws Exception {
        JSONObject before=new JSONObject(ServerSettings.read(new File(context().getFilesDir(),"install-flow-before.json")));
        try(ActivityScenario<MainActivity> activity=ActivityScenario.launch(MainActivity.class)) {
            InstrumentationRegistry.getInstrumentation().waitForIdleSync();
            JSONObject after=installed();assertEquals(before.getLong("target"),after.getLong("code"));
            String installedApkSha=CoreDownload.sha256(new File(context().getApplicationInfo().sourceDir));assertEquals("system installed exact candidate bytes",before.getString("candidate_sha256"),installedApkSha);
            assertEquals(before.getJSONObject("installed").getLong("first_install_time"),after.getLong("first_install_time"));
            assertEquals(before.getJSONObject("installed").getJSONArray("signatures").toString(),after.getJSONArray("signatures").toString());
            JSONObject expected=before.getJSONObject("retained"),actual=retained();assertEquals(expected.length(),actual.length());
            java.util.Iterator<String> keys=expected.keys();while(keys.hasNext()) { String key=keys.next();assertEquals(key,expected.getString(key),actual.getString(key)); }
            assertTrue(HostState.idle());assertFalse(AppUpdateStore.hasInstallation());assertFalse(AppUpdateStore.busy);assertEquals(0,AppUpdateStore.pending(context()).getLong("code",0));
            assertTrue(context().getPackageManager().getPackageInstaller().getMySessions().isEmpty());
            write("install-flow-after.json",new JSONObject().put("status","passed").put("api",Build.VERSION.SDK_INT).put("installed",after).put("installed_apk_sha256",installedApkSha).put("retained",actual).put("reservation_cleared",true).put("host_idle",true));
        }
    }
}
