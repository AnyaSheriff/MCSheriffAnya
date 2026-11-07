package ru.sheriffanya.mcsa;

import android.Manifest;
import android.app.Activity;
import android.app.AlertDialog;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.content.res.ColorStateList;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.text.InputType;
import android.text.SpannableStringBuilder;
import android.text.Spanned;
import android.text.style.ForegroundColorSpan;
import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.widget.ArrayAdapter;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.Spinner;
import android.widget.Switch;
import android.widget.TextView;
import android.widget.Toast;
import java.io.File;
import java.io.IOException;
import java.net.Inet4Address;
import java.net.InetAddress;
import java.net.NetworkInterface;
import java.util.Enumeration;
import java.util.LinkedHashMap;
import java.util.Map;

/** Лёгкий интерфейс на системных Android-компонентах, без WebView и терминала. */
public final class MainActivity extends Activity {
    private static final int INK=0xff20332b, GREEN=0xff315f50, MUTED=0xff64756b, BG=0xfff4f6f2;
    private final Handler handler=new Handler(Looper.getMainLooper());
    private LinearLayout page, container;
    private int tab;
    private String group;
    private TextView status, log;
    private Button start, stop, save, send;
    private ScrollView logScroll;
    private long shownRevision=-1;
    private ServerSettings settings;
    private final Map<String,View> editors=new LinkedHashMap<>();
    private Map<String,String> restoredDraft;
    private final Runnable refresh=new Runnable() {
        @Override public void run() { update(); handler.postDelayed(this,750); }
    };
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        if(state!=null) {
            tab=state.getInt("tab"); group=state.getString("group");
            Bundle draft=state.getBundle("draft");
            if(draft!=null) { restoredDraft=new LinkedHashMap<>(); for(String key:draft.keySet()) restoredDraft.put(key,draft.getString(key)); }
        }
        draw();
    }
    @Override protected void onStart() { super.onStart(); handler.post(refresh); }
    @Override protected void onStop() { handler.removeCallbacks(refresh); super.onStop(); }
    @Override protected void onSaveInstanceState(Bundle state) {
        state.putInt("tab",tab); state.putString("group",group);
        if(settings!=null && !editors.isEmpty()) {
            Bundle draft=new Bundle(); for(Map.Entry<String,String> e:edited().entrySet()) draft.putString(e.getKey(),e.getValue()); state.putBundle("draft",draft);
        }
        super.onSaveInstanceState(state);
    }
    private int dp(int n) { return (int)(n*getResources().getDisplayMetrics().density+0.5f); }
    private TextView text(String value,int size,int color) {
        TextView view=new TextView(this); view.setText(value); view.setTextSize(size); view.setTextColor(color); view.setPadding(0,dp(5),0,dp(5)); return view;
    }
    private Button button(String label,int id,Runnable action) {
        Button button=new Button(this); button.setText(label); if(id!=0) button.setId(id); button.setAllCaps(false);
        button.setStateListAnimator(null);
        tint(button,0xffe3ebe5,GREEN);
        button.setOnClickListener(v->action.run()); return button;
    }
    private void tint(Button button,int background,int foreground) {
        int[][] states=new int[][]{new int[]{-android.R.attr.state_enabled},new int[]{}};
        button.setBackgroundTintList(new ColorStateList(states,new int[]{0xffecefed,background}));
        button.setTextColor(new ColorStateList(states,new int[]{MUTED,foreground}));
    }
    private Button navButton(String label,int id,int selected) {
        Button button=button(label,id,()->navigate(selected)); button.setTextSize(13); button.setMinWidth(0); button.setMinimumWidth(0);
        button.setPadding(dp(2),dp(4),dp(2),dp(4)); button.setSingleLine(true);
        tint(button,tab==selected?GREEN:BG,tab==selected?Color.WHITE:GREEN); return button;
    }
    private LinearLayout column() { LinearLayout view=new LinearLayout(this); view.setOrientation(LinearLayout.VERTICAL); return view; }
    private LinearLayout card() {
        LinearLayout view=column(); view.setPadding(dp(16),dp(12),dp(16),dp(12));
        GradientDrawable bg=new GradientDrawable(); bg.setColor(Color.WHITE); bg.setCornerRadius(dp(18)); view.setBackground(bg);
        LinearLayout.LayoutParams params=new LinearLayout.LayoutParams(-1,-2); params.setMargins(0,dp(8),0,dp(8)); page.addView(view,params); return view;
    }
    private void draw() {
        status=null; log=null; start=null; stop=null; save=null; send=null; editors.clear(); settings=null; shownRevision=-1;
        container=column(); container.setBackgroundColor(BG); container.setPadding(dp(18),dp(12),dp(18),dp(8));
        container.setOnApplyWindowInsetsListener((view,insets)->{
            // В Android15+ target35 рисуется до краёв экрана; элементы остаются за пределами системных панелей.
            view.setPadding(dp(18)+insets.getSystemWindowInsetLeft(),dp(12)+insets.getSystemWindowInsetTop(),dp(18)+insets.getSystemWindowInsetRight(),dp(8)+insets.getSystemWindowInsetBottom());
            return insets;
        });
        TextView title=text("MCSheriffAnya",26,INK); title.setTypeface(null,Typeface.BOLD); container.addView(title);
        container.addView(text("Твой сервер Minecraft · Java 26.3",14,MUTED));
        ScrollView scroll=new ScrollView(this); scroll.setFillViewport(true); page=column(); scroll.addView(page); container.addView(scroll,new LinearLayout.LayoutParams(-1,0,1));
        if(tab==0) home(); else if(tab==1) console(); else preferences();
        LinearLayout nav=new LinearLayout(this);
        nav.addView(navButton("Сервер",R.id.tab_home,0),new LinearLayout.LayoutParams(0,-2,1));
        nav.addView(navButton("Консоль",R.id.tab_console,1),new LinearLayout.LayoutParams(0,-2,1));
        nav.addView(navButton("Настройки",R.id.tab_settings,2),new LinearLayout.LayoutParams(0,-2,1));
        container.addView(nav); setContentView(container); update();
    }
    private void home() {
        LinearLayout info=card(); info.addView(text("СЕРВЕР НА ТЕЛЕФОНЕ",12,GREEN)); status=text("",23,INK); info.addView(status);
        info.addView(text("Ядро работает прямо здесь. Termux и Java устанавливать не нужно.",15,MUTED));
        start=button("Запустить сервер",R.id.start_server,this::askStart); tint(start,GREEN,Color.WHITE); info.addView(start);
        stop=button("Сохранить и остановить",R.id.stop_server,()->action(ServerService.STOP,null)); info.addView(stop);
        LinearLayout address=card(); address.addView(text("Подключение",19,INK));
        try {
            ServerSettings config=new ServerSettings(new File(getFilesDir(),"server"));
            String port=config.server.get("server-port","25565");
            address.addView(text("На этом телефоне: 127.0.0.1:"+port,16,INK));
            String ip=lanAddress();
            if(ip!=null) { String value=ip+":"+port; TextView copy=text("В одной Wi-Fi сети: "+value+"  ⧉",16,GREEN); copy.setOnClickListener(v->{ ((ClipboardManager)getSystemService(CLIPBOARD_SERVICE)).setPrimaryClip(ClipData.newPlainText("Сервер",value)); toast("Адрес скопирован"); }); address.addView(copy); }
            else address.addView(text("Подключись к Wi-Fi, чтобы увидеть адрес для других устройств.",14,MUTED));
            address.addView(text("Bedrock: UDP "+config.mcsa.get("bedrock-port","19132")+"; поддержка клиента частичная.\nАдрес Wi-Fi не является публичным адресом из интернета.",13,MUTED));
            boolean reports=Boolean.parseBoolean(config.mcsa.get("send-error-reports","true"));
            LinearLayout privacy=card(); privacy.addView(text(reports?"Анонимные отчёты включены":"Анонимные отчёты выключены",16,INK));
            privacy.addView(text("Изменить можно в настройках → Дополнительно.",14,MUTED));
        } catch(IOException error) { address.addView(text(error.getMessage(),14,Color.RED)); }
        LinearLayout version=card(); version.addView(text(BuildConfig.RUNTIME_ABI+" · Android 7.0+",14,INK));
        version.addView(text(BuildConfig.RUNTIME_PLUGINS?"WebAssembly-плагины включены.":"В этой сборке WebAssembly-плагины отключены.",14,MUTED));
        version.addView(text("Мир и конфиги сохраняются в данных приложения и переживают обычную остановку и обновление APK с той же подписью.",13,MUTED));
    }
    private void console() {
        page.addView(text("Консоль",23,INK)); page.addView(text("Например: list, tps, say Привет. Команда stop сохранит мир и остановит сервер.",14,MUTED));
        logScroll=new ScrollView(this); log=text("",12,INK); log.setId(R.id.console_log); log.setTypeface(Typeface.MONOSPACE); log.setTextIsSelectable(true); logScroll.addView(log); page.addView(logScroll,new LinearLayout.LayoutParams(-1,dp(360)));
        EditText input=new EditText(this); input.setId(R.id.console_input); input.setHint("Команда серверу"); input.setSingleLine(); input.setImeOptions(EditorInfo.IME_ACTION_SEND); page.addView(input);
        Runnable submit=()->{ String value=input.getText().toString(); if(!value.trim().isEmpty()) { action(ServerService.COMMAND,value); input.setText(""); } };
        send=button("Отправить",R.id.console_send,submit); page.addView(send); input.setOnEditorActionListener((v,id,event)->{ if(id==EditorInfo.IME_ACTION_SEND) { submit.run(); return true; } return false; });
    }
    private void preferences() {
        page.addView(text("Настройки",23,INK));
        page.addView(text("Изменения применяются при следующем запуске. Перед сохранением останови сервер.",14,MUTED));
        try { settings=new ServerSettings(new File(getFilesDir(),"server")); }
        catch(IOException error) { page.addView(text(error.getMessage(),16,Color.RED)); return; }
        if(group==null) {
            for(String name:new String[]{"Сервер","Мир","Вход","Производительность","Дополнительно"}) {
                LinearLayout card=card(); card.addView(button(name+"  ›",0,()->{ group=name; draw(); }));
            }
            page.addView(text("Версия игры: 26.3. Комментарии и остальные параметры в конфиге сохраняются.",14,MUTED)); return;
        }
        page.addView(button("‹ Все разделы",0,()->leave(()->{group=null;draw();})));
        page.addView(text(group,20,INK));
        Map<String,String> values=restoredDraft==null?settings.values():restoredDraft; restoredDraft=null;
        for(ServerSettings.Field f:ServerSettings.FIELDS) if(f.group.equals(group)) {
            LinearLayout card=card(); card.addView(text(f.title,17,INK)); card.addView(text(f.help,13,MUTED));
            String value=values.containsKey(f.key)?values.get(f.key):settings.value(f); View editor;
            if(f.toggle()) { Switch toggle=new Switch(this); toggle.setText("Включено"); toggle.setChecked(Boolean.parseBoolean(value)); editor=toggle; }
            else if(f.choices!=null) {
                Spinner spinner=new Spinner(this); spinner.setAdapter(new ArrayAdapter<>(this,android.R.layout.simple_spinner_dropdown_item,f.labels));
                for(int i=0;i<f.choices.length;i++) if(f.choices[i].equals(value)) spinner.setSelection(i); editor=spinner;
            } else {
                EditText input=new EditText(this); input.setSingleLine(); input.setText(value);
                if(f.min!=Integer.MIN_VALUE) input.setInputType(InputType.TYPE_CLASS_NUMBER|InputType.TYPE_NUMBER_FLAG_SIGNED); editor=input;
            }
            editor.setTag(f.key); editors.put(f.key,editor); card.addView(editor);
        }
        save=button("Сохранить настройки",R.id.save_settings,()->{
            try { settings.save(edited()); toast("Сохранено. Настройки применятся при следующем запуске."); draw(); }
            catch(IOException|IllegalArgumentException error) { alert(error.getMessage()); }
        }); page.addView(save);
    }
    private Map<String,String> edited() {
        Map<String,String> values=settings.values();
        for(ServerSettings.Field f:ServerSettings.FIELDS) {
            View v=editors.get(f.key); if(v==null) continue;
            if(v instanceof Switch) values.put(f.key,Boolean.toString(((Switch)v).isChecked()));
            else if(v instanceof Spinner) values.put(f.key,f.choices[((Spinner)v).getSelectedItemPosition()]);
            else values.put(f.key,((EditText)v).getText().toString());
        }
        return values;
    }
    private void leave(Runnable next) {
        if(settings!=null && !editors.isEmpty() && !edited().equals(settings.values())) {
            new AlertDialog.Builder(this).setTitle("Есть несохранённые изменения").setMessage("Остаться в настройках или выйти без сохранения?")
                .setNegativeButton("Остаться",null).setPositiveButton("Не сохранять",(d,w)->next.run()).show();
        } else next.run();
    }
    private void navigate(int next) { leave(()->{ tab=next; group=null; restoredDraft=null; draw(); }); }
    @Override public void onBackPressed() { if(tab==2 && group!=null) leave(()->{group=null;draw();}); else leave(super::onBackPressed); }
    private void askStart() {
        if(!HostState.idle()) return;
        if(Build.VERSION.SDK_INT>=33 && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)!=PackageManager.PERMISSION_GRANTED)
            requestPermissions(new String[]{Manifest.permission.POST_NOTIFICATIONS},10);
        else launch();
    }
    @Override public void onRequestPermissionsResult(int request,String[] permissions,int[] grants) { super.onRequestPermissionsResult(request,permissions,grants); if(request==10) launch(); }
    private void launch() {
        try { Intent intent=new Intent(this,ServerService.class).setAction(ServerService.START); if(Build.VERSION.SDK_INT>=26) startForegroundService(intent); else startService(intent); }
        catch(RuntimeException error) { alert("Не удалось запустить сервер: "+error.getMessage()); }
    }
    private void action(String action,String value) { Intent intent=new Intent(this,ServerService.class).setAction(action); if(value!=null) intent.putExtra("command",value); startService(intent); }
    private void update() {
        HostState.Phase phase; String message; long revision;
        synchronized(HostState.LOCK) { phase=HostState.phase;message=HostState.message;revision=HostState.revision; }
        boolean idle=phase==HostState.Phase.STOPPED;
        if(status!=null) status.setText(message);
        if(start!=null) start.setEnabled(idle);
        if(stop!=null) stop.setEnabled(!idle && phase!=HostState.Phase.STOPPING);
        if(save!=null) save.setEnabled(idle);
        if(send!=null) send.setEnabled(phase==HostState.Phase.RUNNING);
        for(Map.Entry<String,View> e:editors.entrySet()) e.getValue().setEnabled(idle && (!e.getKey().equals("level-seed") || !new File(getFilesDir(),"server/world").exists()));
        if(log!=null && shownRevision!=revision) {
            boolean bottom=logScroll.getScrollY()+logScroll.getHeight()>=log.getHeight()-dp(24);
            String value=HostState.logs(); SpannableStringBuilder styled=new SpannableStringBuilder(value); int offset=0;
            for(String line:value.split("\n",-1)) {
                int color=line.contains("/ERROR]")?0xffa93430:line.contains("/WARN]")?0xff946300:line.startsWith("› ")?GREEN:INK;
                if(!line.isEmpty()) styled.setSpan(new ForegroundColorSpan(color),offset,offset+line.length(),Spanned.SPAN_EXCLUSIVE_EXCLUSIVE); offset+=line.length()+1;
            }
            log.setText(styled); shownRevision=revision; if(bottom) logScroll.post(()->logScroll.fullScroll(View.FOCUS_DOWN));
        }
    }
    private String lanAddress() {
        try {
            Enumeration<NetworkInterface> interfaces=NetworkInterface.getNetworkInterfaces();
            while(interfaces.hasMoreElements()) { NetworkInterface network=interfaces.nextElement(); if(!network.isUp()||network.isLoopback()) continue;
                Enumeration<InetAddress> addresses=network.getInetAddresses(); while(addresses.hasMoreElements()) { InetAddress address=addresses.nextElement(); if(address instanceof Inet4Address && address.isSiteLocalAddress()) return address.getHostAddress(); }
            }
        } catch(Exception ignored) { }
        return null;
    }
    private void toast(String text) { Toast.makeText(this,text,Toast.LENGTH_LONG).show(); }
    private void alert(String text) { new AlertDialog.Builder(this).setMessage(text).setPositiveButton("Понятно",null).show(); }
}
