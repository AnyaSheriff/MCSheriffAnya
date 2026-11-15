package ru.sheriffanya.mcsa;

import android.content.Context;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.os.Build;
import androidx.test.core.app.ApplicationProvider;
import org.junit.Test;
import java.io.IOException;
import static org.junit.Assert.*;

/** PackageManager читает настоящий установленный APK; архив не распаковывается. */
public class AppUpdatePackageTest {
    @SuppressWarnings("deprecation")
    @Test public void installedApkUsesActualCurrentSignerAndRejectsAnotherApp() throws Exception {
        Context context=ApplicationProvider.getApplicationContext();PackageManager pm=context.getPackageManager();
        int flags=Build.VERSION.SDK_INT>=28?PackageManager.GET_SIGNING_CERTIFICATES:PackageManager.GET_SIGNATURES;
        PackageInfo installed=pm.getPackageInfo(context.getPackageName(),flags);
        PackageInfo archive=pm.getPackageArchiveInfo(context.getApplicationInfo().sourceDir,flags);
        assertNotNull(archive);long code=Build.VERSION.SDK_INT>=28?archive.getLongVersionCode():archive.versionCode;
        AppUpdatePolicy.identity(archive.packageName,code,archive.versionName,archive.versionName,code,AppUpdateStore.signatures(installed),AppUpdateStore.signatures(archive));
        String[] platformSigners=AppUpdateStore.signatures(pm.getPackageInfo("android",flags));
        assertNotNull(platformSigners);assertTrue(platformSigners.length>0);
        try { AppUpdatePolicy.identity(archive.packageName,code,archive.versionName,archive.versionName,code,AppUpdateStore.signatures(installed),platformSigners);fail("foreign real signer accepted"); }
        catch(IOException expected) { assertTrue(expected.getMessage().contains("другим ключом")); }
    }
}
