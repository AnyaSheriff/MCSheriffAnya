package ru.sheriffanya.mcsa;

import android.content.Context;
import androidx.test.core.app.ApplicationProvider;
import androidx.test.platform.app.InstrumentationRegistry;
import org.junit.Test;
import org.junit.Assume;
import org.json.JSONObject;
import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import static org.junit.Assert.*;

/** CI передаёт собственный APK с code+1 и тем же debug signer; без распаковки. */
public class AppUpdateArchiveTest {
    @Test public void actualSignedUpgradeAndHashCodeAbiFailures() throws Exception {
        String path=InstrumentationRegistry.getArguments().getString("appUpgradeApk");
        Assume.assumeTrue("Для отдельной CI проверки нужен appUpgradeApk",path!=null);
        File apk=new File(path);assertTrue("APK fixture is readable",apk.isFile()&&apk.canRead());
        Context app=ApplicationProvider.getApplicationContext();String sha=CoreDownload.sha256(apk);
        AppUpdateStore.verify(app,apk,metadata(apk,BuildConfig.VERSION_CODE+1,BuildConfig.RUNTIME_ABI,sha));
        reject(app,apk,metadata(apk,BuildConfig.VERSION_CODE+2,BuildConfig.RUNTIME_ABI,sha));
        reject(app,apk,metadata(apk,BuildConfig.VERSION_CODE+1,BuildConfig.RUNTIME_ABI,sha.substring(0,63)+(sha.endsWith("0")?"1":"0")));
        String anotherAbi=BuildConfig.RUNTIME_ABI.equals("x86_64")?"arm64-v8a":"x86_64";
        reject(app,apk,metadata(apk,BuildConfig.VERSION_CODE+1,anotherAbi,sha));
    }
    private AppUpdateManifest metadata(File apk,long code,String abi,String sha) throws Exception {
        JSONObject file=new JSONObject().put("bytes",apk.length()).put("sha256",sha).put("url","https://github.com/AnyaSheriff/MCSheriffAnya/releases/download/test/test.apk");
        JSONObject json=new JSONObject().put("format",1).put("package","ru.sheriffanya.mcsa").put("version_code",code).put("version_name",BuildConfig.VERSION_NAME).put("apks",new JSONObject().put(abi,file));
        return AppUpdateManifest.parse(json.toString().getBytes(StandardCharsets.UTF_8),abi,BuildConfig.VERSION_CODE);
    }
    private void reject(Context app,File apk,AppUpdateManifest manifest) throws Exception {
        try { AppUpdateStore.verify(app,apk,manifest);fail("invalid APK accepted"); }catch(IOException expected) { assertNotNull(expected.getMessage()); }
    }
}
