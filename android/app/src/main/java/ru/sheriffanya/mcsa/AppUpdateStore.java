package ru.sheriffanya.mcsa;

import android.content.Context;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.content.pm.Signature;
import android.os.Build;
import java.io.*;
import java.net.URI;
import java.util.UUID;
import java.util.concurrent.Executors;
import javax.net.ssl.HttpsURLConnection;

/** Загрузка не меняет приложение и мир. Установку всегда подтверждает Android. */
final class AppUpdateStore {
    static final String CHANNEL="https://raw.githubusercontent.com/AnyaSheriff/MCSheriffAnya/android-updates/app-preview.json";
    private static final java.util.concurrent.ExecutorService WORK=Executors.newSingleThreadExecutor();
    static volatile boolean busy;private static boolean cacheLoaded;static volatile String message="Проверь обновления приложения";
    static volatile AppUpdateManifest available,readyManifest;static volatile File ready;
    static final String INSTALL_OPERATION="Устанавливаем приложение…";
    static final class Installation {
        final File apk;final AppUpdateManifest manifest;int sessionId=-1;
        Installation(File file,AppUpdateManifest description) { apk=file;manifest=description; }
    }
    private static Installation installation;
    private static boolean verifyingInstallation,installerLaunched;
    static android.content.SharedPreferences pending(Context context) { return context.getSharedPreferences("app-install-pending",Context.MODE_PRIVATE); }
    /** Вызывается Application до запуска служб: старый процесс мог погибнуть в установщике. */
    static synchronized void recoverInstallation(Context context) {
        android.content.SharedPreferences state=pending(context);long target=state.getLong("code",0);
        if(target==0) return;
        if(target<=BuildConfig.VERSION_CODE) { state.edit().clear().commit();return; }
        try {
            String name=state.getString("file","");if(!name.matches("[0-9a-f-]{36}\\.apk")) throw new IOException("Неверное имя APK");
            AppUpdateManifest manifest=AppUpdateManifest.parse(state.getString("manifest","").getBytes(java.nio.charset.StandardCharsets.UTF_8),BuildConfig.RUNTIME_ABI,BuildConfig.VERSION_CODE);
            if(manifest.code!=target) throw new IOException("Неверная версия ожидающего APK");
            installation=new Installation(new File(new File(context.getCacheDir(),"app-updates"),name),manifest);
            installation.sessionId=state.getInt("session",-1);
            ready=installation.apk;readyManifest=manifest;busy=true;installerLaunched=true;verifyingInstallation=false;
            synchronized(HostState.LOCK) { HostState.operation=INSTALL_OPERATION; }
            message="Установка приложения не завершена. Продолжи её и дождись результата Android.";
        } catch(Exception invalid) { state.edit().clear().commit();message="Ожидание установки повреждено. Скачай приложение ещё раз."; }
    }
    static synchronized boolean hasInstallation() { return installation!=null; }
    static synchronized boolean canInstall() { return installation==null&&!busy&&ready!=null&&HostState.idle(); }
    static synchronized Installation currentInstallation() { return installation; }
    static synchronized boolean canReconcileInstallation() { return installation!=null&&!verifyingInstallation; }
    /** Один захват одновременно блокирует замену APK и запуск сервера/копий/ядра. */
    static synchronized Installation beginInstallation(Context context) throws IOException {
        if(installation!=null) throw new IOException("Дождись результата установщика Android");
        if(busy||ready==null||readyManifest==null) throw new IOException("Дождись завершения загрузки APK");
        synchronized(HostState.LOCK) {
            if(!HostState.idle()) throw new IOException("Сначала останови сервер и дождись завершения операций");
            if(!pending(context).edit().putLong("code",readyManifest.code).putString("file",ready.getName()).putString("manifest",new String(readyManifest.raw,java.nio.charset.StandardCharsets.UTF_8)).commit()) throw new IOException("Не удалось сохранить ожидание установки");
            installation=new Installation(ready,readyManifest);busy=true;verifyingInstallation=true;installerLaunched=false;HostState.operation=INSTALL_OPERATION;
        }
        message=INSTALL_OPERATION;return installation;
    }
    static synchronized void sessionCreated(Context context,Installation ticket,int id) throws IOException {
        if(installation!=ticket||!verifyingInstallation) throw new IOException("Ожидание установки изменилось");
        if(!pending(context).edit().putInt("session",id).commit()) throw new IOException("Не удалось сохранить установочную сессию");
        ticket.sessionId=id;
    }
    static synchronized void installerLaunched(Installation ticket) throws IOException {
        if(installation!=ticket||!verifyingInstallation) throw new IOException("Ожидание установки изменилось");
        installerLaunched=true;verifyingInstallation=false;
    }
    static synchronized void finishInstallation(Context context,Installation ticket,String outcome) {
        if(installation==null||(ticket!=null&&ticket!=installation)) return;
        // Удаляем маркер до снятия блокировки: повторный запуск не возобновит старую установку.
        if(!pending(context).edit().clear().commit()) { message="Не удалось завершить ожидание установки. Попробуй ещё раз.";installerLaunched=false;verifyingInstallation=false;return; }
        installation=null;verifyingInstallation=false;installerLaunched=false;busy=false;message=outcome;
        AppInstallSession.clearConfirmation();
        synchronized(HostState.LOCK) { if(INSTALL_OPERATION.equals(HostState.operation)) HostState.operation=null; }
    }
    interface Result { void done(); }
    static synchronized void restore(Context context,Result result) {
        if(cacheLoaded||busy) return;cacheLoaded=true;busy=true;
        Context app=context.getApplicationContext();WORK.execute(()->{
            try {
                File folder=new File(app.getCacheDir(),"app-updates");File[] files=folder.listFiles((dir,name)->name.matches("[0-9a-f-]{36}\\.apk\\.json"));
                if(files!=null) { int count=0;for(File description:files) {
                    if(++count>32) break;
                    try {
                        if(description.length()>65536) continue;byte[] raw;
                        try(InputStream in=new FileInputStream(description);ByteArrayOutputStream out=new ByteArrayOutputStream()) {
                            byte[] buffer=new byte[4096];int n;while((n=in.read(buffer))!=-1) { if(out.size()+n>65536) throw new IOException("Большое описание APK");out.write(buffer,0,n); }raw=out.toByteArray();
                        }
                        AppUpdateManifest manifest=AppUpdateManifest.parse(raw,BuildConfig.RUNTIME_ABI,BuildConfig.VERSION_CODE);
                        File apk=new File(folder,description.getName().substring(0,description.getName().length()-5));verify(app,apk,manifest);
                        if(readyManifest==null||manifest.code>readyManifest.code) { readyManifest=manifest;ready=apk;message="Ранее скачанное приложение проверено. Можно установить."; }
                    } catch(Exception invalid) { /* Повреждённый кэш не пригоден для установки. */ }
                } }
            } finally { busy=false;new android.os.Handler(android.os.Looper.getMainLooper()).post(result::done); }
        });
    }
    static synchronized void check(Context context,Result result) {
        if(busy) return;busy=true;message="Проверяем обновления приложения…";
        WORK.execute(()->{
            try { available=AppUpdateManifest.parse(read(CHANNEL,65536),BuildConfig.RUNTIME_ABI,BuildConfig.VERSION_CODE);message="Доступно приложение "+available.version; }
            catch(Exception error) { message=friendly(error); }
            finally { busy=false;new android.os.Handler(android.os.Looper.getMainLooper()).post(result::done); }
        });
    }
    static synchronized void download(Context context,Result result) {
        if(busy||available==null) return;busy=true;message="Скачиваем приложение…";
        Context app=context.getApplicationContext();AppUpdateManifest manifest=available;
        WORK.execute(()->{File staging=null;
            try {
                File folder=new File(app.getCacheDir(),"app-updates");if(!folder.isDirectory()&&!folder.mkdirs()) throw new IOException("Не удалось создать папку загрузки");
                staging=new File(folder,UUID.randomUUID()+".apk");downloadFile(manifest,staging);
                if(!staging.setReadOnly()) throw new IOException("Не удалось защитить скачанный APK");
                verify(app,staging,manifest);
                File description=new File(staging.getAbsolutePath()+".json");
                try(FileOutputStream out=new FileOutputStream(description)) { out.write(manifest.raw);out.getFD().sync(); }
                File previous=ready;readyManifest=manifest;ready=staging;staging=null;if(previous!=null&&!previous.equals(ready)) { previous.delete();new File(previous.getAbsolutePath()+".json").delete(); }
                message="Приложение скачано и проверено. Можно установить.";
            } catch(Exception error) { message=friendly(error); }
            finally { if(staging!=null) { staging.delete();new File(staging.getAbsolutePath()+".json").delete(); }busy=false;new android.os.Handler(android.os.Looper.getMainLooper()).post(result::done); }
        });
    }
    static void verify(Context context,File apk,AppUpdateManifest manifest) throws Exception {
        if(!manifest.abi.equals(BuildConfig.RUNTIME_ABI)||manifest.code<=BuildConfig.VERSION_CODE||apk.length()!=manifest.bytes||!CoreDownload.sha256(apk).equals(manifest.sha256)) throw new IOException("Проверка целостности APK не пройдена");
        PackageManager pm=context.getPackageManager();int flags=Build.VERSION.SDK_INT>=28?PackageManager.GET_SIGNING_CERTIFICATES:PackageManager.GET_SIGNATURES;
        PackageInfo candidate=pm.getPackageArchiveInfo(apk.getAbsolutePath(),flags|PackageManager.GET_META_DATA),installed=pm.getPackageInfo(context.getPackageName(),flags);
        if(candidate==null) throw new IOException("Android не распознал APK");
        if(candidate.applicationInfo==null||candidate.applicationInfo.metaData==null||!manifest.abi.equals(candidate.applicationInfo.metaData.getString("ru.sheriffanya.mcsa.RUNTIME_ABI"))) throw new IOException("APK предназначен для другой архитектуры");
        long code=Build.VERSION.SDK_INT>=28?candidate.getLongVersionCode():candidate.versionCode;
        AppUpdatePolicy.identity(candidate.packageName,code,candidate.versionName==null?"":candidate.versionName,manifest.version,manifest.code,signatures(installed),signatures(candidate));
    }
    @SuppressWarnings("deprecation")
    static String[] signatures(PackageInfo info) {
        Signature[] signatures=Build.VERSION.SDK_INT>=28?(info.signingInfo==null?null:info.signingInfo.getApkContentsSigners()):info.signatures;
        if(signatures==null) return null;String[] result=new String[signatures.length];for(int i=0;i<result.length;i++) result[i]=signatures[i].toCharsString();return result;
    }
    private static HttpsURLConnection open(String address) throws IOException {
        URI uri=AppUpdatePolicy.https(address);
        for(int i=0;i<6;i++) {
            HttpsURLConnection connection=(HttpsURLConnection)uri.toURL().openConnection();connection.setConnectTimeout(15000);connection.setReadTimeout(15000);connection.setInstanceFollowRedirects(false);connection.setUseCaches(false);connection.setRequestProperty("Accept-Encoding","identity");
            int code;try { code=connection.getResponseCode(); }catch(IOException error) { connection.disconnect();throw error; }
            if(code==200) return connection;String location=connection.getHeaderField("Location");connection.disconnect();
            if(code==404) throw new IOException("Обновления приложения пока не опубликованы");
            if(location==null||(code!=301&&code!=302&&code!=303&&code!=307&&code!=308)) throw new IOException("Сервер обновлений ответил ошибкой "+code);
            uri=AppUpdatePolicy.https(uri.resolve(location).toString());
        }throw new IOException("Слишком много перенаправлений");
    }
    private static byte[] read(String address,int limit) throws IOException {
        HttpsURLConnection connection=open(address);
        try(InputStream in=connection.getInputStream();ByteArrayOutputStream out=new ByteArrayOutputStream()) { byte[] buffer=new byte[4096];int n;while((n=in.read(buffer))!=-1) { if(out.size()+n>limit) throw new IOException("Описание обновления слишком велико");out.write(buffer,0,n); }return out.toByteArray(); }
        finally { connection.disconnect(); }
    }
    private static void downloadFile(AppUpdateManifest manifest,File target) throws Exception {
        HttpsURLConnection connection=open(manifest.url);
        try {
            long length=connection.getContentLengthLong();if(length>=0&&length!=manifest.bytes) throw new IOException("Размер APK не совпадает с описанием");
            try(InputStream in=connection.getInputStream();FileOutputStream out=new FileOutputStream(target)) {
                byte[] buffer=new byte[65536];long total=0;int n;
                while((n=in.read(buffer))!=-1) { total+=n;if(total>manifest.bytes||total>AppUpdatePolicy.MAX_APK) throw new IOException("APK превышает ожидаемый размер");out.write(buffer,0,n); }
                if(total!=manifest.bytes) throw new IOException("Загрузка APK прервана");out.getFD().sync();
            }
        } finally { connection.disconnect(); }
    }
    private static String friendly(Exception error) {
        if(error instanceof java.net.UnknownHostException||error instanceof java.net.SocketTimeoutException||error instanceof java.net.ConnectException) return "Нет связи с сервером обновлений. Проверь интернет и попробуй ещё раз.";
        if(error instanceof javax.net.ssl.SSLException) return "Не удалось проверить защищённое соединение. Проверь дату телефона и попробуй ещё раз.";
        return error instanceof IOException&&error.getMessage()!=null?error.getMessage():"Не удалось обновить приложение. Проверь интернет и попробуй ещё раз.";
    }
}
