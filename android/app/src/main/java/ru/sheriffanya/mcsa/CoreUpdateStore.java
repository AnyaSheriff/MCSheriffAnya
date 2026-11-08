package ru.sheriffanya.mcsa;

import android.content.Context;
import android.system.Os;
import android.util.AtomicFile;
import org.json.JSONObject;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.UUID;

/** Отдельные неизменяемые слоты ядра. Мир и конфиги никогда не заменяются загрузкой. */
final class CoreUpdateStore {
    static final class Core {
        final File library;final String slot,version,buildId;final int revision;
        Core(File library,String slot,int revision,String version,String buildId) { this.library=library;this.slot=slot;this.revision=revision;this.version=version;this.buildId=buildId; }
    }
    private static final Object STATE_IO=new Object();
    private final Context context;private final File directory;private final byte[] key;
    CoreUpdateStore(Context context) throws IOException {
        this.context=context.getApplicationContext();directory=new File(context.getFilesDir().getCanonicalFile(),"core-updates");
        if(!directory.isDirectory()&&!directory.mkdirs()) throw new IOException("Не удалось подготовить папку обновлений");
        if(!directory.getCanonicalFile().equals(directory.getAbsoluteFile())) throw new IOException("Папка обновлений не должна быть ссылкой");
        try(InputStream in=context.getResources().openRawResource(R.raw.core_update_public_key)) { key=read(in,512); }
    }
    CoreManifest manifest(byte[] raw) throws IOException { return CoreManifest.parse(raw,key,BuildConfig.RUNTIME_ABI,BuildConfig.VERSION_CODE); }
    private Core bundled(boolean verify) throws Exception {
        File file=new File(context.getApplicationInfo().nativeLibraryDir,"libmcsa_core.so");
        if(verify) {
            if(!file.isFile()||!CoreDownload.sha256(file).equals(BuildConfig.BUNDLED_CORE_SHA256)) throw new IOException("Встроенное ядро повреждено. Обнови приложение");
            elf(file);
        }
        return new Core(file,"bundled",BuildConfig.BUNDLED_CORE_REVISION,BuildConfig.BUNDLED_CORE_VERSION,BuildConfig.BUNDLED_CORE_BUILD_ID);
    }
    private String pointer(String name) throws IOException { synchronized(STATE_IO) {
        File file=new File(directory,name);if(!file.exists()&&!new File(file+".bak").exists()) return "bundled";
        try(InputStream in=new AtomicFile(file).openRead()) { String value=new String(read(in,128),StandardCharsets.UTF_8).trim();if(!"bundled".equals(value)&&!value.matches("core-[1-9][0-9]{0,9}-[0-9a-f]{16}")) throw new IOException("Неверный слот ядра");return value; }
    }
    }
    private Core slot(String name,boolean verify) throws Exception {
        if("bundled".equals(name)) return bundled(verify);
        if(!name.matches("core-[1-9][0-9]{0,9}-[0-9a-f]{16}")) throw new IOException("Неверный слот ядра");
        File root=new File(directory,name);if(!root.isDirectory()||!root.getCanonicalFile().equals(root.getAbsoluteFile())) throw new IOException("Файлы обновления недоступны");
        CoreManifest data;
        try(InputStream in=new FileInputStream(new File(root,"manifest.json"))) { data=manifest(read(in,65536)); }
        if(!name.equals(name(data))) throw new IOException("Описание обновления не соответствует слоту");
        File library=new File(root,"core.so");
        if(verify) {
            if(!library.getCanonicalFile().equals(library.getAbsoluteFile())||!library.isFile()||library.length()!=data.bytes||(Os.stat(library.getPath()).st_mode&0222)!=0||!CoreDownload.sha256(library).equals(data.sha256)) throw new IOException("Загруженное ядро не прошло проверку целостности");
            elf(library);
        }
        return new Core(library,name,data.revision,data.version,data.buildId);
    }
    String version() {
        try { return slot(pointer("active"),false).version; }
        catch(Exception unavailable) { return BuildConfig.BUNDLED_CORE_VERSION; }
    }
    int revision() {
        try { return slot(pointer("active"),false).revision; }
        catch(Exception unavailable) { return BuildConfig.BUNDLED_CORE_REVISION; }
    }
    boolean hasPrevious() { try { return !pointer("active").equals(pointer("previous")); } catch(IOException unavailable) { return false; } }
    private static String name(CoreManifest manifest) { return "core-"+manifest.revision+"-"+manifest.sha256.substring(0,16); }
    private File pendingFile() { return new File(directory,"pending.json"); }
    private JSONObject pending() throws Exception { synchronized(STATE_IO) {
        File file=pendingFile();if(!file.exists()&&!new File(file+".bak").exists()) return null;
        try(InputStream in=new AtomicFile(file).openRead()) { return new JSONObject(new String(read(in,4096),StandardCharsets.UTF_8)); }
    }
    }
    private void removePending() { synchronized(STATE_IO) { new AtomicFile(pendingFile()).delete(); } }
    private void writePointer(String pointer,String value) throws IOException { write(new File(directory,pointer),(value+"\n").getBytes(StandardCharsets.UTF_8)); }
    private void writePending(JSONObject value) throws IOException { write(pendingFile(),(value+"\n").getBytes(StandardCharsets.UTF_8)); }
    Core prepareLaunch() throws Exception {
        JSONObject pending;
        try { pending=pending(); } catch(Exception invalid) { removePending();pending=null; }
        String active;
        try { active=pointer("active"); } catch(IOException invalid) { active="invalid"; }
        if(pending!=null&&pending.optBoolean("attempting")&&active.equals(pending.optString("slot"))) {
            restorePrevious("Предыдущий запуск обновлённого ядра не завершился. Возвращена прежняя версия.");pending=null;
        }
        Core selected;
        try { selected=slot(pointer("active"),true); }
        catch(Exception invalid) { selected=restorePrevious("Проверка обновления не пройдена. Используется прежняя версия ядра.");pending=null; }
        if(pending!=null&&selected.slot.equals(pending.optString("slot"))) { pending.put("attempting",true);writePending(pending); }
        return selected;
    }
    void ready(Core selected,String actualBuild) throws Exception {
        if(!selected.buildId.equals(actualBuild)) throw new IOException("Версия запущенного ядра не совпадает с проверенной сборкой");
        JSONObject pending=pending();if(pending!=null&&selected.slot.equals(pending.optString("slot"))) removePending();
        try { cleanup(selected.slot,pointer("previous")); } catch(IOException invalid) { /* Запуск проверен; повреждённый previous не мешает работе. */ }
    }
    boolean failed(Core selected,boolean userStopped) {
        try {
            JSONObject pending=pending();if(pending==null||!selected.slot.equals(pending.optString("slot"))) return false;
            if(userStopped) { pending.put("attempting",false);writePending(pending);return false; }
            restorePrevious("Новое ядро не запустилось. Возвращена прежняя версия. Копия мира сохранена в резервных копиях.");return true;
        } catch(Exception invalid) { return false; }
    }
    private Core restorePrevious(String message) throws Exception {
        Core previous;
        try { previous=slot(pointer("previous"),true); } catch(Exception invalid) { previous=bundled(true); }
        writePointer("active",previous.slot);removePending();
        synchronized(HostState.LOCK) { HostState.coreUpdateMessage=message;HostState.coreVersion=previous.version;HostState.coreUpdateRevision++; }
        return previous;
    }
    Core stage(CoreManifest manifest,CoreDownload.Progress progress) throws Exception {
        int highest=context.getSharedPreferences("core-updates",Context.MODE_PRIVATE).getInt("highest",BuildConfig.BUNDLED_CORE_REVISION);
        if(manifest.revision<highest||manifest.revision<=revision()) throw new IOException("Эта версия ядра уже установлена или устарела");
        String name=name(manifest);File destination=new File(directory,name);
        if(destination.exists()) {
            try { return slot(name,true); }
            catch(Exception invalid) { if(!destination.renameTo(new File(directory,"damaged-"+UUID.randomUUID()))) throw new IOException("Не удалось подготовить место для ядра"); }
        }
        File temporary=new File(directory,"stage-"+UUID.randomUUID());if(!temporary.mkdir()) throw new IOException("Не удалось подготовить загрузку");boolean moved=false;
        try {
            CoreDownload.module(manifest,new File(temporary,"core.so"),progress);
            write(new File(temporary,"manifest.json"),manifest.envelope);if(!new File(temporary,"manifest.json").setReadOnly()) throw new IOException("Не удалось защитить описание обновления");
            if(!temporary.renameTo(destination)) throw new IOException("Не удалось сохранить загруженное ядро");moved=true;return slot(name,true);
        } finally { if(!moved) discard(temporary); }
    }
    void activate(Core selected,String backup) throws Exception {
        // Проверяем ещё раз после ZIP-копирования и перед переключением указателя.
        selected=slot(selected.slot,true);Core old=slot(pointer("active"),true);
        JSONObject pending=new JSONObject().put("slot",selected.slot).put("previous",old.slot).put("backup",backup).put("attempting",false);
        int highest=context.getSharedPreferences("core-updates",Context.MODE_PRIVATE).getInt("highest",BuildConfig.BUNDLED_CORE_REVISION);
        if(!context.getSharedPreferences("core-updates",Context.MODE_PRIVATE).edit().putInt("highest",Math.max(highest,selected.revision)).commit()) throw new IOException("Не удалось сохранить состояние обновления");
        writePointer("previous",old.slot);writePending(pending);writePointer("active",selected.slot);
    }
    void rollback() throws Exception {
        String current=pointer("active"),old=pointer("previous");if(current.equals(old)) throw new IOException("Предыдущей версии пока нет");
        Core verified=slot(old,true);writePointer("active",verified.slot);writePointer("previous",current);removePending();
    }
    private void elf(File file) throws IOException {
        byte[] h=new byte[24];try(DataInputStream in=new DataInputStream(new FileInputStream(file))) { in.readFully(h); }
        int bits=BuildConfig.RUNTIME_ABI.equals("armeabi-v7a")?1:2,machine=BuildConfig.RUNTIME_ABI.equals("armeabi-v7a")?40:BuildConfig.RUNTIME_ABI.equals("arm64-v8a")?183:62;
        if(h[0]!=0x7f||h[1]!='E'||h[2]!='L'||h[3]!='F'||h[4]!=bits||h[5]!=1||(h[16]&255)!=3||h[17]!=0||((h[18]&255)|((h[19]&255)<<8))!=machine) throw new IOException("Файл ядра не подходит этому устройству");
    }
    private void cleanup(String active,String previous) {
        File[] files=directory.listFiles();if(files==null) return;
        for(File file:files) if(file.getName().matches("core-[1-9][0-9]{0,9}-[0-9a-f]{16}")&&!file.getName().equals(active)&&!file.getName().equals(previous)) discard(file);
    }
    private static void discard(File directory) {
        try {
            if(!directory.getCanonicalFile().equals(directory.getAbsoluteFile())) return;
            new File(directory,"core.so").delete();new AtomicFile(new File(directory,"manifest.json")).delete();directory.delete();
        } catch(IOException ignored) { }
    }
    private static byte[] read(InputStream input,int max) throws IOException {
        ByteArrayOutputStream out=new ByteArrayOutputStream();byte[] buffer=new byte[4096];int n;
        while((n=input.read(buffer))!=-1) { if(out.size()+n>max) throw new IOException("Файл состояния обновления слишком велик");out.write(buffer,0,n); }return out.toByteArray();
    }
    private static void write(File path,byte[] bytes) throws IOException { synchronized(STATE_IO) {
        AtomicFile file=new AtomicFile(path);FileOutputStream out=null;
        try { out=file.startWrite();out.write(bytes);out.getFD().sync();file.finishWrite(out); }
        catch(IOException error) { if(out!=null) file.failWrite(out);throw error; }
    }
    }
}
