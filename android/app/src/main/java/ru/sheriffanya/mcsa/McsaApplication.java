package ru.sheriffanya.mcsa;

import android.app.Application;
import androidx.appcompat.app.AppCompatDelegate;
import com.google.android.material.color.DynamicColors;
import com.google.android.material.color.DynamicColorsOptions;

public final class McsaApplication extends Application {
    @Override public void onCreate() {
        super.onCreate();
        // Только быстрые rename: до открытия настроек возвращаем мир после прерванного импорта.
        try { new BackupStore(getFilesDir()).recoverForLaunch(); }
        catch(java.io.IOException error) { synchronized(HostState.LOCK) { HostState.backupMessage="Нужна проверка восстановления: "+error.getMessage(); } }
        AppCompatDelegate.setDefaultNightMode(AppPreferences.nightMode(new AppPreferences(this).theme()));
        // Android docs: “Dynamic Color ... added in Android 12”; на старых ОС остаётся наша палитра.
        DynamicColors.applyToActivitiesIfAvailable(this,new DynamicColorsOptions.Builder()
            .setPrecondition((activity,theme)->new AppPreferences(activity).dynamicColors()).build());
    }
}
