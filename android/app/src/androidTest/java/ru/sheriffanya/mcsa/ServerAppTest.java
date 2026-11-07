package ru.sheriffanya.mcsa;

import android.content.Context;
import android.content.Intent;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.view.View;
import android.view.ViewGroup;
import androidx.test.core.app.ActivityScenario;
import androidx.test.platform.app.InstrumentationRegistry;
import androidx.lifecycle.Lifecycle;
import org.junit.Test;
import static org.junit.Assert.*;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.io.File;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.Map;
import org.json.JSONObject;
import org.json.JSONArray;
import android.os.Build;
import android.system.Os;
import android.system.OsConstants;

public class ServerAppTest {
    private Context context() { return InstrumentationRegistry.getInstrumentation().getTargetContext(); }
    private ServerSettings fresh(String name) throws Exception {
        File root=new File(context().getCacheDir(),"settings-"+name+"-"+System.nanoTime()); assertTrue(root.mkdirs()); return new ServerSettings(root);
    }
    @Test public void invalidValuesAndRunningServerCannotWrite() throws Exception {
        ServerSettings s=fresh("validation"); Map<String,String> values=s.values(); values.put("server-port","70000");
        try { s.save(values); fail(); } catch(IllegalArgumentException expected) { }
        assertFalse(new File(s.root,"config/server.properties").exists());
        synchronized(HostState.LOCK) { HostState.phase=HostState.Phase.RUNNING; }
        try { s.save(s.values()); fail(); } catch(IOException expected) { }
        finally { synchronized(HostState.LOCK) { HostState.phase=HostState.Phase.STOPPED; } }
        assertFalse(new File(s.root,"config/server.properties").exists());
    }
    @Test public void staleReadCannotOverwriteAnExternalEdit() throws Exception {
        ServerSettings s=fresh("stale"); s.save(s.values()); s=new ServerSettings(s.root);
        File file=new File(s.root,"config/mcsa.properties"); String changed=ServerSettings.read(file)+"owner-setting=русский текст\n"; ServerSettings.write(file,changed);
        try { s.save(s.values()); fail(); } catch(IOException expected) { }
        assertEquals(changed,ServerSettings.read(file));
    }
    @Test public void unknownCommentsSurviveAndExistingWorldSeedCannotChange() throws Exception {
        ServerSettings s=fresh("world"); File file=new File(s.root,"config/server.properties"); ServerSettings.write(file,"# от владельца\nowner-unknown=значение\n");
        s=new ServerSettings(s.root); Map<String,String> values=s.values(); values.put("motd","Телефон"); s.save(values);
        assertTrue(ServerSettings.read(file).contains("# от владельца\nowner-unknown=значение\n"));
        assertTrue(new File(s.root,"world").mkdirs()); s=new ServerSettings(s.root); String before=ServerSettings.read(file); values=s.values(); values.put("level-seed","999");
        try { s.save(values); fail(); } catch(IllegalArgumentException expected) { }
        assertEquals(before,ServerSettings.read(file));
    }
    @Test public void secondFileFailureRollsBackTheFirst() throws Exception {
        ServerSettings s=fresh("rollback"); s.save(s.values());
        s=new ServerSettings(s.root,(file,text)->{ if(file.getName().equals("mcsa.properties")) throw new IOException("injected second-file write failure"); ServerSettings.write(file,text); });
        File file=new File(s.root,"config/server.properties"); String before=ServerSettings.read(file);
        // Независимо от .bak/.new реализации AtomicFile на разных Android проверяем отказ второго writer.
        Map<String,String> values=s.values(); values.put("motd","не должно сохраниться");
        try { s.save(values); fail(); } catch(IOException expected) { }
        assertEquals(before,ServerSettings.read(file));
    }
    private interface Condition { boolean done() throws Exception; }
    private void await(String name,Condition condition) throws Exception {
        long deadline=System.currentTimeMillis()+120000;
        while(System.currentTimeMillis()<deadline) { if(condition.done()) return; Thread.sleep(100); }
        fail(name+"\n"+HostState.logs());
    }
    private void click(ActivityScenario<MainActivity> activity,int id) throws Exception {
        // Готовность процесса не равна уже нарисованному состоянию кнопки: UI обновляется раз в750мс.
        long deadline=System.currentTimeMillis()+5000; java.util.concurrent.atomic.AtomicBoolean ready=new java.util.concurrent.atomic.AtomicBoolean();
        while(System.currentTimeMillis()<deadline) {
            activity.onActivity(a->{ View view=a.findViewById(id); ready.set(view!=null && view.isEnabled()); });
            if(ready.get()) {
                activity.onActivity(a->{ View view=a.findViewById(id); assertNotNull(view); assertTrue("button disabled: "+a.getResources().getResourceEntryName(id),view.isEnabled()); assertTrue("click not dispatched",view.performClick()); }); return;
            }
            Thread.sleep(50);
        }
        fail("button never enabled id="+id+"; native phase="+HostState.phase+"\n"+HostState.logs());
    }
    private Button buttonWithText(View view,String text) {
        if(view instanceof Button && text.equals(((Button)view).getText().toString())) return (Button)view;
        if(view instanceof ViewGroup) for(int i=0;i<((ViewGroup)view).getChildCount();i++) { Button found=buttonWithText(((ViewGroup)view).getChildAt(i),text); if(found!=null) return found; }
        return null;
    }
    @Test public void nativeServerStartsFromApkSavesSettingsAndRestarts() throws Exception {
        assertEquals("x86_64",BuildConfig.RUNTIME_ABI);
        JSONObject device=new JSONObject().put("api",Build.VERSION.SDK_INT).put("page_size",Os.sysconf(OsConstants._SC_PAGESIZE)).put("supported_abis",new JSONArray(java.util.Arrays.asList(Build.SUPPORTED_ABIS)));
        ServerSettings.write(new File(context().getFilesDir(),"device-test.json"),device.toString()+"\n");
        ServerSettings s=new ServerSettings(new File(context().getFilesDir(),"server")); Map<String,String> values=s.values();
        values.put("server-port","26184"); values.put("bedrock-port","0"); values.put("online-mode","false"); values.put("offline-license-mode","disabled"); values.put("auth-enabled","false"); values.put("send-error-reports","false"); values.put("view-distance","2"); values.put("simulation-distance","2"); values.put("level-seed","1234"); s.save(values);
        try(ActivityScenario<MainActivity> activity=ActivityScenario.launch(MainActivity.class)) {
            click(activity,R.id.start_server); await("first native startup",()->{ synchronized(HostState.LOCK) { return HostState.phase==HostState.Phase.RUNNING; } });
            JSONObject status=ping(); assertEquals(777,status.getJSONObject("version").getInt("protocol")); assertEquals(10,status.getJSONObject("players").getInt("max"));
            device.put("status_before",status);
            activity.moveToState(Lifecycle.State.CREATED);
            Thread.sleep(500);
            JSONObject background=ping(); assertEquals(777,background.getJSONObject("version").getInt("protocol")); device.put("status_background",background);
            activity.moveToState(Lifecycle.State.RESUMED);
            click(activity,R.id.tab_console);
            activity.onActivity(a->((EditText)a.findViewById(R.id.console_input)).setText("say APK_NATIVE_OK")); click(activity,R.id.console_send);
            await("console command reached the native process",()->HostState.logs().contains("INFO]: [Server] APK_NATIVE_OK"));
            click(activity,R.id.tab_home); click(activity,R.id.stop_server); await("first graceful stop",HostState::idle);
            File marker=new File(s.root,"world/apk-persistence-marker"); assertTrue(marker.getParentFile().isDirectory()); ServerSettings.write(marker,"retained\n");
            assertTrue(new File(s.root,"world/level.dat").length()>0);
            click(activity,R.id.tab_settings);
            activity.onActivity(a->{ Button button=buttonWithText(a.getWindow().getDecorView(),"Сервер  ›"); assertNotNull(button); button.performClick(); });
            activity.onActivity(a->((EditText)a.getWindow().getDecorView().findViewWithTag("max-players")).setText("7"));
            activity.recreate();
            activity.onActivity(a->assertEquals("7",((EditText)a.getWindow().getDecorView().findViewWithTag("max-players")).getText().toString()));
            click(activity,R.id.save_settings);
            assertEquals("7",new ServerSettings(s.root).server.get("max-players",""));
            click(activity,R.id.tab_home); click(activity,R.id.start_server); await("second native startup",()->{ synchronized(HostState.LOCK) { return HostState.phase==HostState.Phase.RUNNING; } });
            JSONObject after=ping(); assertEquals(7,after.getJSONObject("players").getInt("max")); device.put("status_after",after); assertEquals("retained\n",ServerSettings.read(marker));
            click(activity,R.id.stop_server); await("second graceful stop",HostState::idle);
            assertFalse("listening socket remained after stop",listening());
            System.out.println("PASS: installed APK native exec, protocol777 status, console, graceful stop, config UI, rotation draft, real config effect, world folder retained, restart, socket closed");
        } finally {
            try { if(!HostState.idle()) { context().startService(new Intent(context(),ServerService.class).setAction(ServerService.STOP)); await("cleanup",HostState::idle); } }
            finally { ServerSettings.write(new File(context().getFilesDir(),"instrument-native.log"),HostState.logs()); ServerSettings.write(new File(context().getFilesDir(),"device-test.json"),device.toString()+"\n"); }
        }
    }
    private boolean listening() { try(Socket socket=new Socket()) { socket.connect(new InetSocketAddress("127.0.0.1",26184),500); return true; } catch(IOException expected) { return false; } }
    private void varint(OutputStream out,int value) throws IOException { do { int b=value&127;value>>>=7;out.write(value==0?b:b|128); } while(value!=0); }
    private int varint(DataInputStream in) throws IOException { int value=0;for(int n=0;n<5;n++){int b=in.readUnsignedByte();value|=(b&127)<<(n*7);if((b&128)==0)return value;}throw new IOException("varint too long"); }
    private void packet(OutputStream out,byte[] bytes) throws IOException { varint(out,bytes.length);out.write(bytes);out.flush(); }
    private JSONObject ping() throws Exception {
        try(Socket socket=new Socket()) {
            socket.connect(new InetSocketAddress("127.0.0.1",26184),3000); socket.setSoTimeout(5000); OutputStream out=socket.getOutputStream();
            ByteArrayOutputStream handshake=new ByteArrayOutputStream();varint(handshake,0);varint(handshake,777);byte[] address="127.0.0.1".getBytes(StandardCharsets.UTF_8);varint(handshake,address.length);handshake.write(address);handshake.write(26184>>8);handshake.write(26184&255);varint(handshake,1);packet(out,handshake.toByteArray());packet(out,new byte[]{0});
            DataInputStream in=new DataInputStream(socket.getInputStream());int length=varint(in);assertTrue(length>0&&length<1024*1024);assertEquals(0,varint(in));int n=varint(in);assertTrue(n>0&&n<length);byte[] json=new byte[n];in.readFully(json);return new JSONObject(new String(json,StandardCharsets.UTF_8));
        }
    }
}
