package ru.sheriffanya.mcsa;

import android.app.Application;
import androidx.appcompat.app.AppCompatDelegate;

public final class McsaApplication extends Application {
    @Override public void onCreate() {
        super.onCreate();
        // Только быстрые rename: до открытия настроек возвращаем мир после прерванного импорта.
        try { new BackupStore(getFilesDir()).recoverForLaunch(); }
        catch(java.io.IOException error) { synchronized(HostState.LOCK) { HostState.backupMessage="Нужна проверка восстановления: "+error.getMessage(); } }
        AppCompatDelegate.setDefaultNightMode(AppPreferences.nightMode(new AppPreferences(this).theme()));
    }
}
