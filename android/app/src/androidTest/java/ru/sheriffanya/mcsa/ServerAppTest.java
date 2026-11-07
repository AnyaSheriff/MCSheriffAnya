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
    private void openAppearance(ActivityScenario<MainActivity> activity) throws Exception {
        click(activity,R.id.tab_settings);
        activity.onActivity(a->{ Button b=buttonWithText(a.getWindow().getDecorView(),"Оформление  ›"); assertNotNull(b); assertTrue(b.performClick()); });
    }
    private void toggleDynamic(ActivityScenario<MainActivity> activity) {
        activity.onActivity(a->{
            android.widget.CompoundButton toggle=a.findViewById(R.id.dynamic_colors);
            assertNotNull(toggle); assertTrue(toggle.isEnabled()); boolean before=toggle.isChecked();
            // CompoundButton без OnClickListener может вернуть false после настоящего изменения checked.
            toggle.performClick(); assertEquals("switch did not change",!before,toggle.isChecked());
        });
    }
    private void themeReady(ActivityScenario<MainActivity> activity,boolean dark,int selected) throws Exception {
        await("theme recreation",()->{
            java.util.concurrent.atomic.AtomicBoolean ready=new java.util.concurrent.atomic.AtomicBoolean();
            activity.onActivity(a->{ View b=a.findViewById(selected); ready.set(b!=null&&b.isSelected()&&((a.getResources().getConfiguration().uiMode&android.content.res.Configuration.UI_MODE_NIGHT_MASK)==(dark?android.content.res.Configuration.UI_MODE_NIGHT_YES:android.content.res.Configuration.UI_MODE_NIGHT_NO))); });
            return ready.get();
        });
    }
    private void screenshot(String name) throws Exception {
        InstrumentationRegistry.getInstrumentation().waitForIdleSync();
        android.graphics.Bitmap image=InstrumentationRegistry.getInstrumentation().getUiAutomation().takeScreenshot(); assertNotNull(image);
        try(java.io.FileOutputStream out=new java.io.FileOutputStream(new File(context().getFilesDir(),name+".png"))) { assertTrue(image.compress(android.graphics.Bitmap.CompressFormat.PNG,100,out)); }
        finally { image.recycle(); }
    }
    private void systemNight(boolean dark) throws Exception {
        systemNight(dark?android.app.UiModeManager.MODE_NIGHT_YES:android.app.UiModeManager.MODE_NIGHT_NO);
    }
    private void systemNight(int mode) throws Exception {
        android.app.UiModeManager manager=(android.app.UiModeManager)context().getSystemService(Context.UI_MODE_SERVICE);
        if(Build.VERSION.SDK_INT<26) {
            // На API24 нет shell-подкоманды uimode night; публичный API действует глобально начиная с API23.
            manager.setNightMode(mode); assertEquals("system night API did not apply",mode,manager.getNightMode());
        } else {
            String value=mode==android.app.UiModeManager.MODE_NIGHT_YES?"yes":mode==android.app.UiModeManager.MODE_NIGHT_AUTO?"auto":"no";
            android.os.ParcelFileDescriptor fd=InstrumentationRegistry.getInstrumentation().getUiAutomation().executeShellCommand("cmd uimode night "+value);
            try(java.io.InputStream in=new android.os.ParcelFileDescriptor.AutoCloseInputStream(fd); ByteArrayOutputStream out=new ByteArrayOutputStream()) {
                byte[] bytes=new byte[512]; int n; while((n=in.read(bytes))!=-1) out.write(bytes,0,n);
                System.out.println("System night command: "+new String(out.toByteArray(),StandardCharsets.UTF_8));
            }
        }
    }
    @Test public void appearancePersistsAndFollowsSystemWithoutChangingConfigs() throws Exception {
        AppPreferences preferences=new AppPreferences(context());
        String originalTheme=preferences.theme(); boolean originalDynamic=preferences.dynamicColors();
        android.app.UiModeManager manager=(android.app.UiModeManager)context().getSystemService(Context.UI_MODE_SERVICE);
        int originalSystem=manager.getNightMode();
        File root=new File(context().getFilesDir(),"server");
        String beforeServer=ServerSettings.read(new File(root,"config/server.properties")),beforeMcsa=ServerSettings.read(new File(root,"config/mcsa.properties"));
        JSONObject evidence=new JSONObject();
        assertTrue(preferences.saveTheme("light")); assertTrue(preferences.saveDynamicColors(false));
        InstrumentationRegistry.getInstrumentation().runOnMainSync(()->androidx.appcompat.app.AppCompatDelegate.setDefaultNightMode(AppPreferences.nightMode("light")));
        try(ActivityScenario<MainActivity> activity=ActivityScenario.launch(MainActivity.class)) {
            openAppearance(activity); themeReady(activity,false,R.id.theme_light); screenshot("appearance-light");
            click(activity,R.id.tab_home); screenshot("home-light"); openAppearance(activity);
            click(activity,R.id.theme_dark); themeReady(activity,true,R.id.theme_dark); assertEquals("dark",new AppPreferences(context()).theme());
            activity.recreate(); themeReady(activity,true,R.id.theme_dark); screenshot("appearance-dark");
            activity.onActivity(a->{
                int foreground=com.google.android.material.color.MaterialColors.getColor(a,com.google.android.material.R.attr.colorOnSurface,"test");
                int surface=com.google.android.material.color.MaterialColors.getColor(a,com.google.android.material.R.attr.colorSurface,"test");
                assertTrue("dark text must be readable",androidx.core.graphics.ColorUtils.calculateContrast(foreground,surface)>=4.5);
                assertEquals(0xff98d3ba,com.google.android.material.color.MaterialColors.getColor(a,com.google.android.material.R.attr.colorPrimary,"test"));
            });
            click(activity,R.id.tab_home); screenshot("home-dark"); openAppearance(activity);
            boolean available=com.google.android.material.color.DynamicColors.isDynamicColorAvailable();
            activity.onActivity(a->assertEquals(available,a.findViewById(R.id.dynamic_colors).isEnabled()));
            if(available) {
                toggleDynamic(activity); themeReady(activity,true,R.id.theme_dark);
                assertTrue(new AppPreferences(context()).dynamicColors());
                activity.onActivity(a->{
                    android.view.ContextThemeWrapper expected=new android.view.ContextThemeWrapper(a,R.style.AppTheme);
                    expected.getTheme().applyStyle(com.google.android.material.R.style.ThemeOverlay_Material3_DynamicColors_DayNight,true);
                    assertEquals(com.google.android.material.color.MaterialColors.getColor(expected,com.google.android.material.R.attr.colorPrimary,"expected"),com.google.android.material.color.MaterialColors.getColor(a,com.google.android.material.R.attr.colorPrimary,"actual"));
                });
                screenshot("appearance-dynamic");
                toggleDynamic(activity); themeReady(activity,true,R.id.theme_dark); assertFalse(new AppPreferences(context()).dynamicColors());
            }
            click(activity,R.id.theme_system); assertEquals("system",new AppPreferences(context()).theme());
            systemNight(true); themeReady(activity,true,R.id.theme_system); screenshot("appearance-system-dark");
            systemNight(false); themeReady(activity,false,R.id.theme_system); screenshot("appearance-system-light");
            click(activity,R.id.theme_light); themeReady(activity,false,R.id.theme_light);
            // Явно светлая тема не должна следовать последующему затемнению системы.
            systemNight(true); themeReady(activity,false,R.id.theme_light);
            activity.recreate(); themeReady(activity,false,R.id.theme_light);
            assertEquals(beforeServer,ServerSettings.read(new File(root,"config/server.properties")));
            assertEquals(beforeMcsa,ServerSettings.read(new File(root,"config/mcsa.properties")));
            evidence.put("api",Build.VERSION.SDK_INT).put("light",true).put("dark",true).put("system_dark",true).put("system_light",true).put("explicit_ignores_system",true).put("persists_recreation",true).put("dynamic_available",available).put("dynamic_roundtrip",available).put("configs_unchanged",true);
            ServerSettings.write(new File(context().getFilesDir(),"appearance-test.json"),evidence.toString()+"\n");
        } finally {
            // Эмулятор возвращаем к исходному системному режиму, не затрагивая пользовательский ПК.
            systemNight(originalSystem);
            assertTrue(preferences.saveTheme(originalTheme)); assertTrue(preferences.saveDynamicColors(originalDynamic));
            InstrumentationRegistry.getInstrumentation().runOnMainSync(()->androidx.appcompat.app.AppCompatDelegate.setDefaultNightMode(AppPreferences.nightMode(originalTheme)));
        }
    }
    @Test public void expandedSettingsPersistAndRejectInvalidRanges() throws Exception {
        ServerSettings s=fresh("expanded"); Map<String,String> values=s.values();
        assertEquals(28,values.size());
        values.put("auth-mode","chat"); values.put("prefetch-chunks","0"); values.put("prefetch-at-once","2"); values.put("network-compression-threshold","-1"); values.put("portal-delay","20"); values.put("tab-tps","false"); values.put("autumn-forests","false"); values.put("pink-cherry-groves","false"); values.put("prevent-proxy-connections","true"); values.put("text-info","false"); values.put("debug","true");
        s.save(values); ServerSettings loaded=new ServerSettings(s.root);
        assertEquals(values,loaded.values());
        assertEquals("-1",loaded.server.get("network-compression-threshold","missing"));
        assertEquals("chat",loaded.mcsa.get("auth-mode","missing"));
        String sp=ServerSettings.read(new File(s.root,"config/server.properties")),mp=ServerSettings.read(new File(s.root,"config/mcsa.properties"));
        for(String[] invalid:new String[][]{{"prefetch-chunks","17"},{"prefetch-at-once","0"},{"portal-delay","-1"},{"network-compression-threshold","-2"},{"auth-mode","bad"},{"view-distance","2"},{"debug","yes"}}) {
            Map<String,String> changed=loaded.values(); changed.put(invalid[0],invalid[1]);
            try { loaded.save(changed); fail("accepted "+invalid[0]); } catch(IllegalArgumentException expected) { }
            assertEquals(sp,ServerSettings.read(new File(s.root,"config/server.properties"))); assertEquals(mp,ServerSettings.read(new File(s.root,"config/mcsa.properties")));
        }
    }
    @Test public void nativeServerStartsFromApkSavesSettingsAndRestarts() throws Exception {
        assertEquals("x86_64",BuildConfig.RUNTIME_ABI);
        JSONObject device=new JSONObject().put("api",Build.VERSION.SDK_INT).put("page_size",Os.sysconf(OsConstants._SC_PAGESIZE)).put("supported_abis",new JSONArray(java.util.Arrays.asList(Build.SUPPORTED_ABIS)));
        ServerSettings.write(new File(context().getFilesDir(),"device-test.json"),device.toString()+"\n");
        ServerSettings s=new ServerSettings(new File(context().getFilesDir(),"server")); Map<String,String> values=s.values();
        values.put("server-port","26184"); values.put("bedrock-port","0"); values.put("online-mode","false"); values.put("offline-license-mode","disabled"); values.put("auth-enabled","false"); values.put("send-error-reports","false"); values.put("view-distance","3"); values.put("simulation-distance","3"); values.put("level-seed","1234"); s.save(values);
        try(ActivityScenario<MainActivity> activity=ActivityScenario.launch(MainActivity.class)) {
            click(activity,R.id.start_server); await("first native startup",()->{ synchronized(HostState.LOCK) { return HostState.phase==HostState.Phase.RUNNING; } });
            JSONObject status=ping(); assertEquals(777,status.getJSONObject("version").getInt("protocol")); assertEquals(10,status.getJSONObject("players").getInt("max"));
            device.put("status_before",status);
            activity.moveToState(Lifecycle.State.CREATED);
            Thread.sleep(500);
            JSONObject background=ping(); assertEquals(777,background.getJSONObject("version").getInt("protocol")); device.put("status_background",background);
            activity.moveToState(Lifecycle.State.RESUMED);
            String sp=ServerSettings.read(new File(s.root,"config/server.properties")),mp=ServerSettings.read(new File(s.root,"config/mcsa.properties"));
            openAppearance(activity); click(activity,R.id.theme_dark); themeReady(activity,true,R.id.theme_dark);
            assertEquals(777,ping().getJSONObject("version").getInt("protocol"));
            assertEquals(sp,ServerSettings.read(new File(s.root,"config/server.properties"))); assertEquals(mp,ServerSettings.read(new File(s.root,"config/mcsa.properties")));
            device.put("live_theme_change_status",ping()).put("theme_change_configs_unchanged",true);
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
            await("save feedback visible",()->{
                java.util.concurrent.atomic.AtomicBoolean visible=new java.util.concurrent.atomic.AtomicBoolean();
                activity.onActivity(a->{ android.widget.TextView feedback=a.findViewById(com.google.android.material.R.id.snackbar_text); visible.set(feedback!=null&&feedback.isShown()&&feedback.getText().toString().startsWith("Сохранено.")); });
                return visible.get();
            });
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
