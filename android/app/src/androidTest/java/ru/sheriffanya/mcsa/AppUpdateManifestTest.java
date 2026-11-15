package ru.sheriffanya.mcsa;

import org.junit.Test;
import java.nio.charset.StandardCharsets;
import java.io.IOException;
import static org.junit.Assert.*;

public class AppUpdateManifestTest {
    private byte[] manifest(long code,String abi,String url,long bytes,String sha) {
        return ("{\"format\":1,\"package\":\"ru.sheriffanya.mcsa\",\"version_code\":"+code+",\"version_name\":\"test\",\"apks\":{\""+abi+"\":{\"bytes\":"+bytes+",\"sha256\":\""+sha+"\",\"url\":\""+url+"\"}}}").getBytes(StandardCharsets.UTF_8);
    }
    @Test public void strictMetadata() throws Exception {
        String sha="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        String url="https://github.com/AnyaSheriff/MCSheriffAnya/releases/download/test/app.apk";
        assertEquals(6,AppUpdateManifest.parse(manifest(6,"x86_64",url,1000000,sha),"x86_64",5).code);
        reject(manifest(5,"x86_64",url,1000000,sha),"x86_64");
        reject(manifest(6,"arm64-v8a",url,1000000,sha),"x86_64");
        reject(manifest(6,"x86_64",url,AppUpdatePolicy.MAX_APK+1,sha),"x86_64");
        reject(manifest(6,"x86_64",url,1000000,"bad"),"x86_64");
        reject(manifest(6,"x86_64","https://github.com/Other/app.apk",1000000,sha),"x86_64");
        reject(new byte[65537],"x86_64");reject(new byte[0],"x86_64");
    }
    private void reject(byte[] raw,String abi) throws Exception {
        try { AppUpdateManifest.parse(raw,abi,5);fail("invalid manifest accepted"); }catch(IOException expected) { assertNotNull(expected.getMessage()); }
    }
}
