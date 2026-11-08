package ru.sheriffanya.mcsa.otaprobe;

import android.content.Context;
import android.os.Build;
import android.system.Os;
import android.system.OsConstants;
import androidx.test.platform.app.InstrumentationRegistry;
import org.json.JSONObject;
import org.junit.Test;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.concurrent.*;
import static org.junit.Assert.*;

/** Проверяет только поддерживаемую загрузку read-only .so, НЕ игровой процесс. */
public final class LoaderProbeTest {
    private static String hash(File file) throws Exception {
        MessageDigest digest=MessageDigest.getInstance("SHA-256");
        try(InputStream in=new FileInputStream(file)) { byte[] b=new byte[8192];int n;while((n=in.read(b))!=-1) digest.update(b,0,n); }
        StringBuilder out=new StringBuilder();for(byte b:digest.digest()) out.append(String.format(java.util.Locale.ROOT,"%02x",b&255));return out.toString();
    }
    private static File copyReadonly(Context context,File source,String name) throws Exception {
        File target=new File(context.getFilesDir(),name);if(target.exists()) assertTrue(target.delete());
        try(FileOutputStream out=new FileOutputStream(target);InputStream in=new FileInputStream(source)) {
            // Android14 DCL: сначала RO-файл, затем запись через уже открытый дескриптор.
            assertTrue(target.setReadOnly());byte[] b=new byte[8192];int n;while((n=in.read(b))!=-1) out.write(b,0,n);out.getFD().sync();
        }
        assertEquals(0,Os.stat(target.getPath()).st_mode&0222);assertEquals(hash(source),hash(target));return target;
    }
    private static String execute(File host,File module,int expected) throws Exception {
        ProcessBuilder builder=new ProcessBuilder(host.getPath()).redirectErrorStream(true);
        if(module!=null) builder.environment().put("MCSA_CORE_LIBRARY",module.getPath());
        Process process=builder.start();ExecutorService reader=Executors.newSingleThreadExecutor();
        try {
            Future<String> output=reader.submit(()->{
                try(InputStream in=process.getInputStream();ByteArrayOutputStream bytes=new ByteArrayOutputStream()) {
                    byte[] b=new byte[1024];int n;while((n=in.read(b))!=-1) { bytes.write(b,0,n);if(bytes.size()>4096) throw new IOException("probe output exceeded limit"); }
                    return new String(bytes.toByteArray(),StandardCharsets.UTF_8);
                }
            });
            String text=output.get(10,TimeUnit.SECONDS);assertEquals(text,expected,process.waitFor());return text;
        } finally { process.destroy();reader.shutdownNow(); }
    }
    @Test public void readonlyCoreCanChangeWithoutReinstallingApplication() throws Exception {
        Context context=InstrumentationRegistry.getInstrumentation().getTargetContext();File libraryDir=new File(context.getApplicationInfo().nativeLibraryDir);
        File host=new File(libraryDir,"libmcsa.so"),one=new File(libraryDir,"libmcsa_core.so"),two=new File(libraryDir,"libmcsa_core_v2.so");
        assertTrue(host.isFile()&&host.canExecute());assertTrue(one.isFile()&&two.isFile());
        assertTrue(execute(host,null,37).contains("MCSA_CORE_PROBE_37"));
        File first=copyReadonly(context,one,"core-v1.so"),second=copyReadonly(context,two,"core-v2.so");assertNotEquals(hash(first),hash(second));
        assertTrue(execute(host,first,37).contains("MCSA_CORE_PROBE_37"));assertTrue(execute(host,second,38).contains("MCSA_CORE_PROBE_38"));
        File broken=new File(context.getFilesDir(),"invalid.so");if(broken.exists()) assertTrue(broken.delete());try(FileOutputStream out=new FileOutputStream(broken)) { assertTrue(broken.setReadOnly());out.write("invalid ELF".getBytes(StandardCharsets.UTF_8));out.getFD().sync(); }
        assertTrue(execute(host,broken,126).contains("загрузка ядра не удалась"));
        JSONObject proof=new JSONObject().put("api",Build.VERSION.SDK_INT).put("page_size",Os.sysconf(OsConstants._SC_PAGESIZE)).put("abi",Build.SUPPORTED_ABIS[0])
            .put("bundled_exit",37).put("private_v1_exit",37).put("private_v2_exit",38).put("invalid_exit",126).put("private_v1_sha256",hash(first)).put("private_v2_sha256",hash(second))
            .put("writable_bits",Os.stat(second.getPath()).st_mode&0222).put("gameplay_tested",false);
        try(FileOutputStream out=new FileOutputStream(new File(context.getFilesDir(),"probe-result.json"))) { out.write((proof+"\n").getBytes(StandardCharsets.UTF_8));out.getFD().sync(); }
    }
}
