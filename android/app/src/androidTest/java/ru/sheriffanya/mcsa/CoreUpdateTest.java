package ru.sheriffanya.mcsa;

import android.content.Context;
import android.content.Intent;
import android.os.Build;
import android.system.Os;
import android.util.Base64;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import androidx.test.core.app.ActivityScenario;
import androidx.test.platform.app.InstrumentationRegistry;
import org.json.JSONObject;
import org.junit.Test;
import static org.junit.Assert.*;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.security.*;
import java.security.spec.ECGenParameterSpec;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.zip.ZipFile;

/** Только наш установленный APK: настоящий HTTPS, полное ядро и сохранённый мир. */
public class CoreUpdateTest {
    private Context context() { return InstrumentationRegistry.getInstrumentation().getTargetContext(); }
    private interface Condition { boolean ready() throws Exception; }
    private void await(String name,Condition condition) throws Exception {
        long end=System.currentTimeMillis()+180000;
        while(System.currentTimeMillis()<end) { if(condition.ready()) return;Thread.sleep(100); }
        fail(name+"; "+HostState.message+"; "+HostState.coreUpdateMessage+"\n"+HostState.logs());
    }
    private void click(ActivityScenario<MainActivity> activity,int id) throws Exception {
        await("enabled button "+id,()->{ AtomicBoolean enabled=new AtomicBoolean();activity.onActivity(a->{ View v=a.findViewById(id);enabled.set(v!=null&&v.isEnabled()); });return enabled.get(); });
        activity.onActivity(a->{ View v=a.findViewById(id);assertNotNull(v);assertTrue(v.isEnabled());assertTrue(v.performClick()); });
    }
    private Button textButton(View view,String text) {
        if(view instanceof Button&&text.equals(((Button)view).getText().toString())) return (Button)view;
        if(view instanceof ViewGroup) for(int i=0;i<((ViewGroup)view).getChildCount();i++) { Button b=textButton(((ViewGroup)view).getChildAt(i),text);if(b!=null)return b; }return null;
    }
    private void openUpdates(ActivityScenario<MainActivity> activity) throws Exception {
        click(activity,R.id.tab_settings);
        activity.onActivity(a->{ Button b=textButton(a.getWindow().getDecorView(),"Обновления ядра  ›");assertNotNull(b);assertTrue(b.performClick()); });
    }
    private void confirm(String caption) {
        androidx.test.uiautomator.UiDevice ui=androidx.test.uiautomator.UiDevice.getInstance(InstrumentationRegistry.getInstrumentation());
        java.util.regex.Pattern pattern=java.util.regex.Pattern.compile(caption,java.util.regex.Pattern.CASE_INSENSITIVE|java.util.regex.Pattern.UNICODE_CASE);
        androidx.test.uiautomator.UiObject2 b=ui.wait(androidx.test.uiautomator.Until.findObject(androidx.test.uiautomator.By.text(pattern).clickable(true)),10000);assertNotNull("confirmation",b);b.click();
    }
    private void screenshot(String name) throws Exception {
        InstrumentationRegistry.getInstrumentation().waitForIdleSync();
        android.graphics.Bitmap image=InstrumentationRegistry.getInstrumentation().getUiAutomation().takeScreenshot();assertNotNull(image);
        try(FileOutputStream out=new FileOutputStream(new File(context().getFilesDir(),name+".png"))) { assertTrue(image.compress(android.graphics.Bitmap.CompressFormat.PNG,100,out)); }finally { image.recycle(); }
    }
    private void start(ActivityScenario<MainActivity> activity,String id) throws Exception {
        click(activity,R.id.tab_home);click(activity,R.id.start_server);
        await("real core ready "+id,()->{ synchronized(HostState.LOCK) { return HostState.phase==HostState.Phase.RUNNING; } });
        assertTrue(HostState.logs().contains("MCSA_CORE_BUILD="+id));assertEquals(777,ping().getJSONObject("version").getInt("protocol"));assertEquals(7,ping().getJSONObject("players").getInt("max"));
    }
    private void stop(ActivityScenario<MainActivity> activity) throws Exception { click(activity,R.id.tab_home);click(activity,R.id.stop_server);await("native graceful stop",HostState::idle); }
    private void retained(File marker) throws Exception { assertEquals("retained\n",ServerSettings.read(marker));assertEquals("7",new ServerSettings(new File(context().getFilesDir(),"server")).server.get("max-players","")); }
    @Test public void fullCoreUpdatesOverHttpsWithoutReinstallAndCanRecover() throws Exception {
        assertEquals("x86_64",BuildConfig.RUNTIME_ABI);assertEquals(1,BuildConfig.CORE_HOST_API);assertTrue(HostState.idle());
        CoreUpdateStore store=new CoreUpdateStore(context());assertEquals(BuildConfig.BUNDLED_CORE_REVISION,store.revision());
        File marker=new File(context().getFilesDir(),"server/world/apk-persistence-marker");retained(marker);
        android.content.pm.PackageInfo before=context().getPackageManager().getPackageInfo(context().getPackageName(),0);
        String apkSha=CoreDownload.sha256(new File(context().getApplicationInfo().sourceDir));
        BackupStore backups=new BackupStore(context().getFilesDir());int copies=backups.list().size();
        JSONObject proof=new JSONObject().put("api",Build.VERSION.SDK_INT).put("abi",BuildConfig.RUNTIME_ABI).put("bundled_build",BuildConfig.BUNDLED_CORE_BUILD_ID).put("apk_sha256",apkSha);
        try(ActivityScenario<MainActivity> activity=ActivityScenario.launch(MainActivity.class)) {
            start(activity,BuildConfig.BUNDLED_CORE_BUILD_ID);
            openUpdates(activity);click(activity,R.id.core_check_updates);
            await("real HTTPS signed manifest",()->{ synchronized(HostState.LOCK) { return HostState.operation==null&&HostState.coreAvailable!=null; } });
            CoreManifest manifest;synchronized(HostState.LOCK) { manifest=HostState.coreAvailable; }
            assertEquals(BuildConfig.BUNDLED_CORE_REVISION+1,manifest.revision);assertEquals(BuildConfig.BUNDLED_CORE_BUILD_ID.replace("-r"+BuildConfig.BUNDLED_CORE_REVISION,"-r"+(BuildConfig.BUNDLED_CORE_REVISION+1)),manifest.buildId);
            activity.onActivity(a->{ assertFalse(a.findViewById(R.id.core_apply_update).isEnabled());assertFalse(a.findViewById(R.id.core_rollback).isEnabled()); });
            context().startService(new Intent(context(),CoreUpdateService.class).setAction(CoreUpdateService.INSTALL));Thread.sleep(250);assertEquals(BuildConfig.BUNDLED_CORE_REVISION,store.revision());assertEquals(copies,backups.list().size());
            stop(activity);openUpdates(activity);click(activity,R.id.core_apply_update);confirm("Обновить");
            await("actual HTTPS module verified and activated",()->HostState.idle()&&store.revision()==BuildConfig.BUNDLED_CORE_REVISION+1);
            assertEquals(copies+1,backups.list().size());retained(marker);assertTrue(store.hasPrevious());
            CoreUpdateStore.Core selected=store.prepareLaunch();assertEquals(manifest.sha256,CoreDownload.sha256(selected.library));assertEquals(0,Os.stat(selected.library.getPath()).st_mode&0222);
            // prepareLaunch помечает попытку старта. Пользовательская отмена оставляет её повторяемой.
            assertFalse(store.failed(selected,true));assertEquals(BuildConfig.BUNDLED_CORE_REVISION+1,store.revision());
            File copy=backups.list().get(0);
            try(ZipFile zip=new ZipFile(copy);InputStream in=zip.getInputStream(zip.getEntry("server/world/apk-persistence-marker"))) { byte[] bytes=new byte[9];assertEquals(9,in.read(bytes));assertEquals("retained\n",new String(bytes,StandardCharsets.UTF_8)); }
            screenshot("ota-ready");start(activity,manifest.buildId);retained(marker);proof.put("updated_status",ping());stop(activity);
            assertFalse(new File(context().getFilesDir(),"core-updates/pending.json").exists());
            openUpdates(activity);click(activity,R.id.core_rollback);confirm("Вернуть");
            await("UI rollback",()->HostState.idle()&&store.revision()==BuildConfig.BUNDLED_CORE_REVISION);assertEquals(copies+2,backups.list().size());retained(marker);
            start(activity,BuildConfig.BUNDLED_CORE_BUILD_ID);stop(activity);
            // Повторное применение сохранённого подписанного слота, затем имитация прерванного старта.
            CoreUpdateStore.Core candidate=store.stage(manifest,(done,total)->{});store.activate(candidate,copy.getName());store.prepareLaunch();
            assertEquals("bundled",new CoreUpdateStore(context()).prepareLaunch().slot);assertEquals(BuildConfig.BUNDLED_CORE_REVISION,store.revision());retained(marker);
            store.activate(candidate,copy.getName());CoreUpdateStore.Core attempted=store.prepareLaunch();assertTrue(store.failed(attempted,false));assertEquals(BuildConfig.BUNDLED_CORE_REVISION,store.revision());
            // Подмена уже скачанного файла: проверка до dlopen возвращает встроенный модуль.
            File saved=new File(context().getCacheDir(),"ota-core-original.so");copy(candidate.library,saved);
            store.activate(candidate,copy.getName());assertTrue(candidate.library.setWritable(true,true));
            try(RandomAccessFile out=new RandomAccessFile(candidate.library,"rw")) { out.seek(128);out.write(out.read()^0xff); }assertTrue(candidate.library.setReadOnly());
            assertEquals("bundled",store.prepareLaunch().slot);retained(marker);
            assertTrue(candidate.library.delete());copyReadOnly(saved,candidate.library);assertTrue(saved.delete());
            store.activate(candidate,copy.getName());store.prepareLaunch();store.failed(candidate,true);
            start(activity,manifest.buildId);stop(activity);retained(marker);
            try { store.stage(manifest,(done,total)->{});fail("replayed current revision"); } catch(IOException expected) { }
            File active=new File(context().getFilesDir(),"core-updates/active");ServerSettings.write(active,"../../server\n");assertEquals("bundled",store.prepareLaunch().slot);retained(marker);
            // Снова работающая OTA-версия; вручную слоты изменялись только для отказных проверок.
            store.activate(candidate,copy.getName());store.prepareLaunch();store.failed(candidate,true);start(activity,manifest.buildId);stop(activity);
            android.content.pm.PackageInfo after=context().getPackageManager().getPackageInfo(context().getPackageName(),0);
            assertEquals(before.firstInstallTime,after.firstInstallTime);assertEquals(before.lastUpdateTime,after.lastUpdateTime);assertEquals(apkSha,CoreDownload.sha256(new File(context().getApplicationInfo().sourceDir)));
            proof.put("updated_build",manifest.buildId).put("updated_sha256",manifest.sha256).put("module_bytes",manifest.bytes).put("manifest_https",BuildConfig.CORE_MANIFEST_URL).put("module_https",manifest.url).put("no_apk_reinstall",true).put("world_retained",true).put("settings_retained",true).put("automatic_backup",true).put("ui_rollback",true).put("corrupt_module_rejected",true).put("interrupted_start_recovered",true).put("failed_start_recovered",true).put("running_install_rejected",true).put("replay_rejected",true).put("corrupt_pointer_recovered",true).put("read_only",true).put("first_install",after.firstInstallTime).put("last_update",after.lastUpdateTime);
            openUpdates(activity);screenshot("ota-updated");
        } finally {
            if(!HostState.idle()) { context().startService(new Intent(context(),ServerService.class).setAction(ServerService.STOP));await("cleanup",HostState::idle); }
            ServerSettings.write(new File(context().getFilesDir(),"ota-native.log"),HostState.logs());
        }
        ServerSettings.write(new File(context().getFilesDir(),"ota-test.json"),proof.toString()+"\n");
    }
    private static void copy(File source,File destination) throws Exception { try(InputStream in=new FileInputStream(source);OutputStream out=new FileOutputStream(destination)) { byte[] buffer=new byte[65536];int n;while((n=in.read(buffer))!=-1)out.write(buffer,0,n); } }
    private static void copyReadOnly(File source,File destination) throws Exception { try(InputStream in=new FileInputStream(source);FileOutputStream out=new FileOutputStream(destination)) { assertTrue(destination.setReadOnly());byte[] buffer=new byte[65536];int n;while((n=in.read(buffer))!=-1)out.write(buffer,0,n);out.getFD().sync(); } }
    private byte[] envelope(JSONObject payload,PrivateKey key) throws Exception {
        byte[] raw=payload.toString().getBytes(StandardCharsets.UTF_8);Signature s=Signature.getInstance("SHA256withECDSA");s.initSign(key);s.update(raw);
        return new JSONObject().put("payload",Base64.encodeToString(raw,Base64.NO_WRAP)).put("signature",Base64.encodeToString(s.sign(),Base64.NO_WRAP)).toString().getBytes(StandardCharsets.UTF_8);
    }
    @Test public void rejectsTamperWrongSignerIncompatibleAndUnsafeMetadata() throws Exception {
        // Порт fixture не443. Проверяем тот же системный HTTPS trust store напрямую;
        // production downloader дополнительно запрещает нестандартные порты ещё до TLS.
        javax.net.ssl.HttpsURLConnection untrusted=(javax.net.ssl.HttpsURLConnection)new URL("https://10.0.2.2:26447/test").openConnection();
        untrusted.setConnectTimeout(5000);untrusted.setReadTimeout(5000);
        try { untrusted.getInputStream().close();fail("untrusted TLS certificate accepted"); }
        catch(javax.net.ssl.SSLHandshakeException expected) {
            Throwable cause=expected;boolean trustFailure=false;
            while(cause!=null) { if(cause instanceof java.security.cert.CertPathValidatorException)trustFailure=true;cause=cause.getCause(); }
            assertTrue("TLS failed for another reason",trustFailure);
        } finally { untrusted.disconnect(); }
        byte[] real=CoreDownload.manifest(BuildConfig.CORE_MANIFEST_URL);CoreUpdateStore store=new CoreUpdateStore(context());CoreManifest parsed=store.manifest(real);assertEquals(BuildConfig.BUNDLED_CORE_REVISION+1,parsed.revision);
        JSONObject outer=new JSONObject(new String(real,StandardCharsets.UTF_8));JSONObject data=new JSONObject(new String(Base64.decode(outer.getString("payload"),Base64.DEFAULT),StandardCharsets.UTF_8));
        outer.put("payload",Base64.encodeToString(data.put("version","tampered").toString().getBytes(StandardCharsets.UTF_8),Base64.NO_WRAP));
        try { store.manifest(outer.toString().getBytes(StandardCharsets.UTF_8));fail("tampered signature accepted"); }catch(IOException expected) { }
        KeyPairGenerator generator=KeyPairGenerator.getInstance("EC");generator.initialize(new ECGenParameterSpec("secp256r1"));KeyPair pair=generator.generateKeyPair();
        byte[] signed=envelope(data,pair.getPrivate());assertNotNull(CoreManifest.parse(signed,pair.getPublic().getEncoded(),"x86_64",4));
        try { store.manifest(signed);fail("untrusted signer accepted"); }catch(IOException expected) { }
        for(String field:new String[]{"schema","channel","game","data_compat","revision","min_app","host_api","source","build_id"}) {
            JSONObject changed=new JSONObject(data.toString());Object value=field.equals("source")?"bad":field.equals("build_id")?"wrong":field.equals("channel")?"other":field.equals("game")?"1.12.2":field.equals("revision")?0:999;
            changed.put(field,value);try { CoreManifest.parse(envelope(changed,pair.getPrivate()),pair.getPublic().getEncoded(),"x86_64",4);fail("accepted "+field); }catch(IOException expected) { }
        }
        for(String address:new String[]{"http://example.com/core","https://user:password@example.com/core","https://example.com:444/core","file:///data/core","https://example.com/core#fragment"}) {
            JSONObject changed=new JSONObject(data.toString());changed.getJSONObject("packages").getJSONObject("x86_64").put("url",address);
            try { CoreManifest.parse(envelope(changed,pair.getPrivate()),pair.getPublic().getEncoded(),"x86_64",4);fail("accepted unsafe URL"); }catch(IOException expected) { }
        }
        for(long size:new long[]{0,CoreManifest.MAX_MODULE+1}) {
            JSONObject changed=new JSONObject(data.toString());changed.getJSONObject("packages").getJSONObject("x86_64").put("bytes",size);
            try { CoreManifest.parse(envelope(changed,pair.getPrivate()),pair.getPublic().getEncoded(),"x86_64",4);fail("accepted invalid size"); }catch(IOException expected) { }
        }
        try { CoreManifest.parse(signed,pair.getPublic().getEncoded(),"wrong-abi",4);fail("accepted ABI"); }catch(IOException expected) { }
        ServerSettings.write(new File(context().getFilesDir(),"ota-security.json"),new JSONObject().put("api",Build.VERSION.SDK_INT).put("untrusted_tls_rejected",true).put("tampered_signature_rejected",true).put("wrong_signer_rejected",true).put("incompatible_fields_rejected",9).put("unsafe_urls_rejected",5).put("invalid_sizes_rejected",2).put("wrong_abi_rejected",true).toString()+"\n");
    }
    private static void varint(OutputStream out,int value) throws IOException { do { int b=value&127;value>>>=7;out.write(value==0?b:b|128); }while(value!=0); }
    private static int varint(DataInputStream in) throws IOException { int v=0;for(int n=0;n<5;n++) { int b=in.readUnsignedByte();v|=(b&127)<<(n*7);if((b&128)==0)return v; }throw new IOException("varint"); }
    private static void packet(OutputStream out,byte[] bytes) throws IOException { varint(out,bytes.length);out.write(bytes);out.flush(); }
    private JSONObject ping() throws Exception {
        try(Socket socket=new Socket()) {
            socket.connect(new InetSocketAddress("127.0.0.1",26184),3000);socket.setSoTimeout(5000);OutputStream out=socket.getOutputStream();ByteArrayOutputStream handshake=new ByteArrayOutputStream();
            varint(handshake,0);varint(handshake,777);byte[] address="127.0.0.1".getBytes(StandardCharsets.UTF_8);varint(handshake,address.length);handshake.write(address);handshake.write(26184>>8);handshake.write(26184&255);varint(handshake,1);packet(out,handshake.toByteArray());packet(out,new byte[]{0});
            DataInputStream in=new DataInputStream(socket.getInputStream());int length=varint(in);assertTrue(length>0&&length<1024*1024);assertEquals(0,varint(in));int n=varint(in);assertTrue(n>0&&n<length);byte[] bytes=new byte[n];in.readFully(bytes);return new JSONObject(new String(bytes,StandardCharsets.UTF_8));
        }
    }
}
