package ru.sheriffanya.mcsa;

import android.app.ActivityManager;
import android.content.Context;
import android.net.LocalSocket;
import android.net.LocalSocketAddress;
import android.os.StatFs;
import android.os.SystemClock;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.*;
import org.json.JSONObject;

/** Read-only JSON API своего процесса через сокет в приватной папке приложения. */
final class CoreStatsClient implements AutoCloseable {
    static final class Snapshot {
        final String session;final int online,maxPlayers,logicalCpus;
        final long uptimeMs,cpuUs,rssBytes,at,availableRam,totalRam,availableDisk,tick;
        final double tps,mspt,cpuPercent;final boolean paused,frozen;
        Snapshot(String session,int online,int max,int cpus,long uptime,long cpu,long rss,long at,double tps,double mspt,boolean paused,boolean frozen,long tick,double percent,long availableRam,long totalRam,long disk) {
            this.session=session;this.online=online;maxPlayers=max;logicalCpus=cpus;uptimeMs=uptime;cpuUs=cpu;rssBytes=rss;this.at=at;this.tps=tps;this.mspt=mspt;this.paused=paused;this.frozen=frozen;this.tick=tick;cpuPercent=percent;this.availableRam=availableRam;this.totalRam=totalRam;availableDisk=disk;
        }
    }
    private final Context context;private final String session;private final File socket;
    private final ScheduledExecutorService worker=Executors.newSingleThreadScheduledExecutor(r->new Thread(r,"mcsa-stats"));
    private volatile boolean closed;private volatile LocalSocket connection;private Snapshot previous;
    CoreStatsClient(Context context,File socket,String session) { this.context=context.getApplicationContext();this.socket=socket;this.session=session; }
    static File socket(Context context) { return new File(context.getFilesDir(),"mcsa-api.sock"); }
    void start() { worker.scheduleWithFixedDelay(this::poll,250,2000,TimeUnit.MILLISECONDS); }
    private void poll() {
        synchronized(HostState.LOCK) { if(closed||!HostState.dashboardVisible||HostState.phase!=HostState.Phase.RUNNING) { previous=null;return; } }
        try {
            JSONObject data;
            try(LocalSocket local=new LocalSocket()) {
                connection=local;local.setSoTimeout(1200);local.connect(new LocalSocketAddress(socket.getPath(),LocalSocketAddress.Namespace.FILESYSTEM));
                local.getOutputStream().write("status\n".getBytes(StandardCharsets.UTF_8));local.getOutputStream().flush();
                ByteArrayOutputStream bytes=new ByteArrayOutputStream();byte[] buffer=new byte[1024];int n;
                while((n=local.getInputStream().read(buffer))!=-1) { bytes.write(buffer,0,n);if(bytes.size()>8192) throw new IOException("Слишком большой ответ статистики"); }
                data=new JSONObject(new String(bytes.toByteArray(),StandardCharsets.UTF_8));
            } finally { connection=null; }
            Snapshot current=decode(context,data,session,previous);previous=current;
            synchronized(HostState.LOCK) { if(!closed&&session.equals(HostState.apiSession)&&HostState.phase==HostState.Phase.RUNNING) HostState.stats=current; }
        } catch(Exception unavailable) { /* Без лог-спама и выдуманных нулей: UI отмечает устаревшие данные. */ }
    }
    static Snapshot decode(Context context,JSONObject data,String session,Snapshot previous) throws Exception {
        if(data.getInt("version")!=1||!session.equals(data.getString("session"))||data.getLong("pid")<=0) throw new IOException("Неверная сессия API");
        int online=data.getInt("online"),max=data.getInt("max_players"),cpus=data.getInt("logical_cpus");long uptime=data.getLong("uptime_ms"),cpu=data.isNull("cpu_time_us")?-1:data.getLong("cpu_time_us"),rss=data.isNull("rss_bytes")?-1:data.getLong("rss_bytes");
        if(online<0||max<1||cpus<1||uptime<0||cpu< -1||rss< -1) throw new IOException("Некорректная статистика");
        double tps=number(data,"tps"),mspt=number(data,"mspt");
        double percent=previous!=null&&session.equals(previous.session)?cpuPercent(previous.cpuUs,cpu,previous.uptimeMs,uptime,cpus):Double.NaN;
        long available=-1,total=-1,disk=-1;
        try { ActivityManager.MemoryInfo memory=new ActivityManager.MemoryInfo();((ActivityManager)context.getSystemService(Context.ACTIVITY_SERVICE)).getMemoryInfo(memory);available=memory.availMem;total=memory.totalMem; } catch(RuntimeException ignored) { }
        try { disk=new StatFs(context.getFilesDir().getPath()).getAvailableBytes(); } catch(RuntimeException ignored) { }
        return new Snapshot(session,online,max,cpus,uptime,cpu,rss,SystemClock.elapsedRealtime(),tps,mspt,data.getBoolean("paused"),data.optBoolean("frozen",false),data.getLong("tick"),percent,available,total,disk);
    }
    private static double number(JSONObject data,String key) throws Exception {
        if(data.isNull(key)) return Double.NaN;double value=data.getDouble(key);
        if(Double.isNaN(value)||Double.isInfinite(value)||value<0) throw new IOException("Неверный показатель "+key);return value;
    }
    static double cpuPercent(long before,long after,long fromMs,long toMs,int cpus) {
        if(before<0||after<before||fromMs<0||toMs<=fromMs||cpus<1) return Double.NaN;
        return Math.max(0,Math.min(100,100.0*(after-before)/(1000.0*(toMs-fromMs)*cpus)));
    }
    @Override public void close() {
        closed=true;worker.shutdownNow();LocalSocket local=connection;if(local!=null) try { local.close(); } catch(IOException ignored) { }
    }
}
