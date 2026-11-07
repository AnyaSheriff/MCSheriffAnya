package ru.sheriffanya.mcsa;

import android.Manifest;
import androidx.appcompat.app.AppCompatActivity;
import androidx.appcompat.app.AppCompatDelegate;
import androidx.appcompat.widget.AppCompatEditText;
import androidx.appcompat.widget.AppCompatSpinner;
import androidx.core.view.WindowInsetsControllerCompat;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.card.MaterialCardView;
import com.google.android.material.color.MaterialColors;
import com.google.android.material.color.DynamicColors;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import com.google.android.material.materialswitch.MaterialSwitch;
import com.google.android.material.textfield.TextInputLayout;
import android.content.res.Configuration;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.content.res.ColorStateList;
import android.graphics.Typeface;
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
import android.widget.CompoundButton;
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

/** Нативный Material3-интерфейс без WebView и терминала. */
public final class MainActivity extends AppCompatActivity {
    private int INK, GREEN, MUTED, BG, SURFACE, ON_GREEN, CONTAINER, ON_CONTAINER, ERROR, WARNING;
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
        INK=color(com.google.android.material.R.attr.colorOnSurface);
        GREEN=color(com.google.android.material.R.attr.colorPrimary);
        MUTED=color(com.google.android.material.R.attr.colorOnSurfaceVariant);
        BG=color(android.R.attr.colorBackground); SURFACE=color(com.google.android.material.R.attr.colorSurface);
        ON_GREEN=color(com.google.android.material.R.attr.colorOnPrimary);
        CONTAINER=color(com.google.android.material.R.attr.colorPrimaryContainer);
        ON_CONTAINER=color(com.google.android.material.R.attr.colorOnPrimaryContainer);
        ERROR=color(com.google.android.material.R.attr.colorError); WARNING=getColor(R.color.warning);
        getWindow().setStatusBarColor(BG); getWindow().setNavigationBarColor(BG);
        boolean light=(getResources().getConfiguration().uiMode&Configuration.UI_MODE_NIGHT_MASK)!=Configuration.UI_MODE_NIGHT_YES;
        WindowInsetsControllerCompat bars=new WindowInsetsControllerCompat(getWindow(),getWindow().getDecorView());
        bars.setAppearanceLightStatusBars(light); bars.setAppearanceLightNavigationBars(light);
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
    private int color(int attr) { return MaterialColors.getColor(this,attr,"MCSheriffAnya"); }
    private int dp(int n) { return (int)(n*getResources().getDisplayMetrics().density+0.5f); }
    private TextView text(String value,int size,int color) {
        TextView view=new TextView(this); view.setText(value); view.setTextSize(size); view.setTextColor(color); view.setPadding(0,dp(5),0,dp(5)); return view;
    }
    private Button button(String label,int id,Runnable action) {
        Button button=new MaterialButton(this); button.setText(label); if(id!=0) button.setId(id); button.setAllCaps(false);
        button.setStateListAnimator(null);
        tint(button,CONTAINER,ON_CONTAINER);
        button.setOnClickListener(v->action.run()); return button;
    }
    private void tint(Button button,int background,int foreground) {
        int[][] states=new int[][]{new int[]{-android.R.attr.state_enabled},new int[]{}};
        button.setBackgroundTintList(new ColorStateList(states,new int[]{MaterialColors.layer(SURFACE,INK,0.12f),background}));
        button.setTextColor(new ColorStateList(states,new int[]{MUTED,foreground}));
    }
    private Button navButton(String label,int id,int selected) {
        Button button=button(label,id,()->navigate(selected)); button.setTextSize(13); button.setMinWidth(0); button.setMinimumWidth(0);
        button.setPadding(dp(2),dp(4),dp(2),dp(4)); button.setSingleLine(true);
        tint(button,tab==selected?GREEN:BG,tab==selected?ON_GREEN:GREEN); return button;
    }
    private LinearLayout column() { LinearLayout view=new LinearLayout(this); view.setOrientation(LinearLayout.VERTICAL); return view; }
    private LinearLayout card() {
        LinearLayout view=column(); view.setPadding(dp(16),dp(12),dp(16),dp(12));
        MaterialCardView shell=new MaterialCardView(this); shell.setRadius(dp(20)); shell.setCardBackgroundColor(SURFACE);
        shell.setCardElevation(0); shell.setStrokeWidth(0); shell.addView(view);
        LinearLayout.LayoutParams params=new LinearLayout.LayoutParams(-1,-2); params.setMargins(0,dp(8),0,dp(8)); page.addView(shell,params); return view;
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
        start=button("Запустить сервер",R.id.start_server,this::askStart); tint(start,GREEN,ON_GREEN); info.addView(start);
        stop=button("Сохранить и остановить",R.id.stop_server,()->action(ServerService.STOP,null)); info.addView(stop);
        LinearLayout address=card(); address.addView(text("Подключение",19,INK));
        try {
            ServerSettings config=new ServerSettings(new File(getFilesDir(),"server"));
            String port=config.server.get("server-port","25565");
            address.addView(text("На этом телефоне: 127.0.0.1:"+port,16,INK));
            String ip=lanAddress();
            if(ip!=null) { String value=ip+":"+port; TextView copy=text("В одной Wi-Fi сети: "+value+"\nНажми, чтобы скопировать",16,GREEN); copy.setOnClickListener(v->{ ((ClipboardManager)getSystemService(CLIPBOARD_SERVICE)).setPrimaryClip(ClipData.newPlainText("Сервер",value)); toast("Адрес скопирован"); }); address.addView(copy); }
            else address.addView(text("Подключись к Wi-Fi, чтобы увидеть адрес для других устройств.",14,MUTED));
            String bedrock=config.mcsa.get("bedrock-port","19132");
            address.addView(text((bedrock.equals("0")?"Bedrock выключен.":"Bedrock: UDP "+bedrock+"; поддержка клиента частичная.")+"\nАдрес Wi-Fi не является публичным адресом из интернета.",13,MUTED));
            boolean reports=Boolean.parseBoolean(config.mcsa.get("send-error-reports","true"));
            LinearLayout privacy=card(); privacy.addView(text(reports?"Анонимные отчёты включены":"Анонимные отчёты выключены",16,INK));
            privacy.addView(text("Изменить можно в настройках → Дополнительно.",14,MUTED));
        } catch(IOException error) { address.addView(text(error.getMessage(),14,ERROR)); }
        LinearLayout version=card(); version.addView(text(BuildConfig.RUNTIME_ABI+" · Android 7.0+",14,INK));
        version.addView(text(BuildConfig.RUNTIME_PLUGINS?"WebAssembly-плагины включены.":"В этой сборке WebAssembly-плагины отключены.",14,MUTED));
        version.addView(text("Мир и конфиги сохраняются в данных приложения и переживают обычную остановку и обновление APK с той же подписью.",13,MUTED));
    }
    private void console() {
        page.addView(text("Консоль",23,INK)); page.addView(text("Например: list, tps, say Привет. Команда stop сохранит мир и остановит сервер.",14,MUTED));
        logScroll=new ScrollView(this); log=text("",12,INK); log.setId(R.id.console_log); log.setTypeface(Typeface.MONOSPACE); log.setTextIsSelectable(true); logScroll.addView(log); page.addView(logScroll,new LinearLayout.LayoutParams(-1,dp(360)));
        EditText input=new AppCompatEditText(this); input.setId(R.id.console_input); input.setHint("Команда серверу"); input.setSingleLine(); input.setImeOptions(EditorInfo.IME_ACTION_SEND); page.addView(input);
        Runnable submit=()->{ String value=input.getText().toString(); if(!value.trim().isEmpty()) { action(ServerService.COMMAND,value); input.setText(""); } };
        send=button("Отправить",R.id.console_send,submit); page.addView(send); input.setOnEditorActionListener((v,id,event)->{ if(id==EditorInfo.IME_ACTION_SEND) { submit.run(); return true; } return false; });
    }
    private void preferences() {
        page.addView(text("Настройки",23,INK));
        if("Оформление".equals(group)) { appearance(); return; }
        page.addView(text("Настройки сервера применятся при следующем запуске. Оформление можно менять во время игры.",14,MUTED));
        try { settings=new ServerSettings(new File(getFilesDir(),"server")); }
        catch(IOException error) { page.addView(text(error.getMessage(),16,ERROR)); return; }
        if(group==null) {
            for(String name:new String[]{"Оформление","Сервер","Мир","Вход","Производительность","Сеть","Дополнительно"}) {
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
            if(f.toggle()) { MaterialSwitch toggle=new MaterialSwitch(this); toggle.setText("Включено"); toggle.setChecked(Boolean.parseBoolean(value)); editor=toggle; }
            else if(f.choices!=null) {
                Spinner spinner=new AppCompatSpinner(this); spinner.setAdapter(new ArrayAdapter<>(this,android.R.layout.simple_spinner_dropdown_item,f.labels));
                for(int i=0;i<f.choices.length;i++) if(f.choices[i].equals(value)) spinner.setSelection(i); editor=spinner;
            } else {
                EditText input=new AppCompatEditText(this); input.setSingleLine(); input.setText(value);
                if(f.min!=Integer.MIN_VALUE) input.setInputType(InputType.TYPE_CLASS_NUMBER|InputType.TYPE_NUMBER_FLAG_SIGNED); editor=input;
            }
            editor.setTag(f.key); editors.put(f.key,editor);
            if(editor instanceof EditText) {
                TextInputLayout field=new TextInputLayout(this); field.setHint(f.title); field.setBoxBackgroundMode(TextInputLayout.BOX_BACKGROUND_OUTLINE);
                field.addView(editor,new LinearLayout.LayoutParams(-1,-2)); card.addView(field);
            } else card.addView(editor);
        }
        save=button("Сохранить настройки",R.id.save_settings,()->{
            try { settings.save(edited()); toast("Сохранено. Настройки применятся при следующем запуске."); draw(); }
            catch(IOException|IllegalArgumentException error) { alert(error.getMessage()); }
        }); page.addView(save);
    }
    private void appearance() {
        page.addView(button("‹ Все разделы",0,()->{group=null;draw();}));
        page.addView(text("Оформление",20,INK));
        page.addView(text("Применяется сразу. Сервер продолжит работать, а мир и конфиги останутся на месте.",14,MUTED));
        AppPreferences preferences=new AppPreferences(this);
        LinearLayout theme=card(); theme.addView(text("Тема приложения",18,INK));
        String[] values={"system","light","dark"}, labels={"Как в системе","Светлая","Тёмная"};
        int[] ids={R.id.theme_system,R.id.theme_light,R.id.theme_dark};
        for(int i=0;i<values.length;i++) {
            String value=values[i]; Button choice=button(labels[i],ids[i],()->{
                if(value.equals(preferences.theme())) return;
                if(!preferences.saveTheme(value)) { alert("Не удалось сохранить оформление"); return; }
                int mode=AppPreferences.nightMode(value);
                AppCompatDelegate.setDefaultNightMode(mode);
                // Политика может измениться без смены фактической яркости: обновляем выбранную кнопку и в этом случае.
                draw();
            });
            choice.setSelected(value.equals(preferences.theme()));
            choice.setContentDescription(labels[i]+(choice.isSelected()?", выбрана":""));
            if(choice.isSelected()) tint(choice,GREEN,ON_GREEN);
            theme.addView(choice);
        }
        LinearLayout colors=card(); colors.addView(text("Цвета Material You",18,INK));
        boolean available=DynamicColors.isDynamicColorAvailable();
        colors.addView(text(available?"Палитра из обоев и системного оформления телефона.":"На этом устройстве используется палитра приложения. Системные цвета доступны на совместимых устройствах с Android 12 и новее.",14,MUTED));
        MaterialSwitch dynamic=new MaterialSwitch(this); dynamic.setId(R.id.dynamic_colors); dynamic.setText("Использовать системные цвета");
        dynamic.setChecked(preferences.dynamicColors()); dynamic.setEnabled(available);
        dynamic.setOnCheckedChangeListener((v,checked)->{
            if(!preferences.saveDynamicColors(checked)) { dynamic.setOnCheckedChangeListener(null); dynamic.setChecked(!checked); alert("Не удалось сохранить оформление"); return; }
            recreate();
        }); colors.addView(dynamic);
        page.addView(text("Material Design 3 · MCSheriffAnya "+BuildConfig.VERSION_NAME,13,MUTED));
    }
    private Map<String,String> edited() {
        Map<String,String> values=settings.values();
        for(ServerSettings.Field f:ServerSettings.FIELDS) {
            View v=editors.get(f.key); if(v==null) continue;
            if(v instanceof CompoundButton) values.put(f.key,Boolean.toString(((CompoundButton)v).isChecked()));
            else if(v instanceof Spinner) values.put(f.key,f.choices[((Spinner)v).getSelectedItemPosition()]);
            else values.put(f.key,((EditText)v).getText().toString());
        }
        return values;
    }
    private void leave(Runnable next) {
        if(settings!=null && !editors.isEmpty() && !edited().equals(settings.values())) {
            new MaterialAlertDialogBuilder(this).setTitle("Есть несохранённые изменения").setMessage("Остаться в настройках или выйти без сохранения?")
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
                int color=line.contains("/ERROR]")?ERROR:line.contains("/WARN]")?WARNING:line.startsWith("› ")?GREEN:INK;
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
    private void alert(String text) { new MaterialAlertDialogBuilder(this).setMessage(text).setPositiveButton("Понятно",null).show(); }
}
