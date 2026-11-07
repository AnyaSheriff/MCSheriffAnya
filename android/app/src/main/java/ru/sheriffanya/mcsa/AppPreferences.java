package ru.sheriffanya.mcsa;

import android.content.Context;
import android.content.SharedPreferences;
import androidx.appcompat.app.AppCompatDelegate;

/** Оформление хранится отдельно от конфигов ядра и меняется без остановки мира. */
final class AppPreferences {
    private final SharedPreferences preferences;
    AppPreferences(Context context) { preferences=context.getSharedPreferences("appearance",Context.MODE_PRIVATE); }
    String theme() { return preferences.getString("theme","system"); }
    boolean dynamicColors() { return preferences.getBoolean("dynamic-colors",true); }
    static int nightMode(String theme) {
        if("light".equals(theme)) return AppCompatDelegate.MODE_NIGHT_NO;
        if("dark".equals(theme)) return AppCompatDelegate.MODE_NIGHT_YES;
        return AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM;
    }
    boolean saveTheme(String value) {
        if(!"system".equals(value)&&!"light".equals(value)&&!"dark".equals(value)) throw new IllegalArgumentException("Неизвестная тема");
        return preferences.edit().putString("theme",value).commit();
    }
    boolean saveDynamicColors(boolean value) { return preferences.edit().putBoolean("dynamic-colors",value).commit(); }
}
