package ru.sheriffanya.mcsa;

import javax.net.ssl.HttpsURLConnection;
import java.io.*;
import java.net.URI;
import java.security.MessageDigest;

/** HTTPS без понижения до HTTP; ограничения действуют и без Content-Length. */
final class CoreDownload {
    interface Progress { void received(long bytes,long total); }
    private static HttpsURLConnection open(String address) throws IOException {
        URI uri=CoreManifest.https(address);
        for(int redirect=0;redirect<=5;redirect++) {
            HttpsURLConnection connection=(HttpsURLConnection)uri.toURL().openConnection();
            connection.setConnectTimeout(15000);connection.setReadTimeout(15000);connection.setInstanceFollowRedirects(false);connection.setUseCaches(false);
            connection.setRequestProperty("Accept-Encoding","identity");connection.setRequestProperty("Cache-Control","no-cache");connection.setRequestProperty("User-Agent","MCSheriffAnya-Android/"+BuildConfig.VERSION_NAME);
            int status;
            try { status=connection.getResponseCode(); } catch(IOException error) { connection.disconnect();throw error; }
            if(status==200) return connection;
            String location=connection.getHeaderField("Location");connection.disconnect();
            if(status==404) throw new IOException("Обновления пока не опубликованы");
            if((status!=301&&status!=302&&status!=303&&status!=307&&status!=308)||location==null) throw new IOException("Сервер обновлений ответил ошибкой "+status);
            uri=CoreManifest.https(uri.resolve(location).toString());
        }
        throw new IOException("Слишком много перенаправлений обновления");
    }
    static byte[] manifest(String address) throws IOException {
        HttpsURLConnection connection=open(address);
        try(InputStream in=connection.getInputStream();ByteArrayOutputStream out=new ByteArrayOutputStream()) {
            byte[] buffer=new byte[4096];int n;while((n=in.read(buffer))!=-1) { if(out.size()+n>65536) throw new IOException("Описание обновления слишком велико");out.write(buffer,0,n); }return out.toByteArray();
        } finally { connection.disconnect(); }
    }
    static void module(CoreManifest manifest,File destination,Progress progress) throws Exception {
        if(destination.exists()) throw new IOException("Файл загрузки уже существует");
        HttpsURLConnection connection=open(manifest.url);boolean complete=false;
        try {
            long length=connection.getContentLengthLong();if(length>=0&&length!=manifest.bytes) throw new IOException("Размер ядра отличается от подписанного описания");
            MessageDigest digest=MessageDigest.getInstance("SHA-256");long total=0,last=0;
            try(InputStream in=connection.getInputStream();FileOutputStream out=new FileOutputStream(destination)) {
                // Дескриптор уже открыт: запись разрешена, а новые открытия видят только RO.
                if(!destination.setReadOnly()) throw new IOException("Не удалось защитить файл ядра от записи");
                byte[] buffer=new byte[65536];int n;while((n=in.read(buffer))!=-1) {
                    total+=n;if(total>manifest.bytes||total>CoreManifest.MAX_MODULE) throw new IOException("Файл ядра превышает ожидаемый размер");
                    out.write(buffer,0,n);digest.update(buffer,0,n);long now=android.os.SystemClock.elapsedRealtime();if(now-last>=250) { progress.received(total,manifest.bytes);last=now; }
                }
                out.getFD().sync();
            }
            if(total!=manifest.bytes||!hex(digest.digest()).equals(manifest.sha256)) throw new IOException("Проверка целостности ядра не пройдена");
            progress.received(total,manifest.bytes);complete=true;
        } finally { connection.disconnect();if(!complete) destination.delete(); }
    }
    static String sha256(File file) throws Exception {
        MessageDigest digest=MessageDigest.getInstance("SHA-256");
        try(InputStream in=new FileInputStream(file)) { byte[] bytes=new byte[65536];int n;while((n=in.read(bytes))!=-1) digest.update(bytes,0,n); }return hex(digest.digest());
    }
    private static String hex(byte[] bytes) { StringBuilder out=new StringBuilder();for(byte b:bytes) out.append(String.format(java.util.Locale.ROOT,"%02x",b&255));return out.toString(); }
}
