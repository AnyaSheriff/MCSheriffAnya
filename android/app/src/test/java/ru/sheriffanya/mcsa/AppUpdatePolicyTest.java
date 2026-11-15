package ru.sheriffanya.mcsa;

import org.junit.Test;
import java.io.IOException;
import static org.junit.Assert.*;

public class AppUpdatePolicyTest {
    @Test public void redirectHostsAreExactAndTlsOnly() throws Exception {
        assertEquals("github.com",AppUpdatePolicy.https("https://github.com/AnyaSheriff/a").getHost());
        assertEquals("release-assets.githubusercontent.com",AppUpdatePolicy.https("https://release-assets.githubusercontent.com/a?token=public").getHost());
        for(String value:new String[]{"http://github.com/a","https://github.com.evil/a","https://evil@github.com/a","https://github.com:444/a","https://github.com/a#b","https://localhost/a","https://raw.githubusercontent.com.evil/a"}) {
            try { AppUpdatePolicy.https(value);fail(value); }catch(IOException expected) { assertNotNull(expected.getMessage()); }
        }
    }
    @Test public void candidateMustMatchPackageVersionAndAllCurrentSigners() throws Exception {
        AppUpdatePolicy.identity("ru.sheriffanya.mcsa",6,"0.2.4-preview","0.2.4-preview",6,new String[]{"a","b"},new String[]{"b","a"});
        reject("elsewhere",6,"v",6,new String[]{"a"},new String[]{"a"});
        reject("ru.sheriffanya.mcsa",5,"v",6,new String[]{"a"},new String[]{"a"});
        reject("ru.sheriffanya.mcsa",6,"different",6,new String[]{"a"},new String[]{"a"});
        reject("ru.sheriffanya.mcsa",6,"v",6,new String[]{"a"},new String[]{"foreign"});
        reject("ru.sheriffanya.mcsa",6,"v",6,new String[]{"a"},new String[]{"a","foreign"});
        reject("ru.sheriffanya.mcsa",6,"v",6,new String[0],new String[0]);
        reject("ru.sheriffanya.mcsa",6,"v",6,null,new String[]{"a"});
    }
    private static void reject(String name,long code,String version,long expected,String[] a,String[] b) throws Exception {
        try { AppUpdatePolicy.identity(name,code,version,"v",expected,a,b);fail("accepted invalid APK"); }catch(IOException correct) { assertNotNull(correct.getMessage()); }
    }
}
