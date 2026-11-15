package ru.sheriffanya.mcsa;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

/** Только явный PendingIntent от нашей установочной сессии; receiver не экспортируется. */
public final class AppInstallReceiver extends BroadcastReceiver {
    @Override public void onReceive(Context context,Intent intent) { AppInstallSession.result(context,intent); }
}
