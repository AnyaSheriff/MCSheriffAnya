package ru.sheriffanya.mcsa;

import org.json.JSONObject;
import java.io.IOException;
import java.nio.charset.StandardCharsets;

final class AppUpdateManifest {
    final long code,bytes;final String version,sha256,url,abi;final byte[] raw;
    private AppUpdateManifest(byte[] input,String selectedAbi,long installedCode) throws Exception {
        JSONObject data=new JSONObject(new String(input,StandardCharsets.UTF_8));
        if(integer(data,"format")!=1||!"ru.sheriffanya.mcsa".equals(data.getString("package"))) throw new IOException("Неизвестный канал приложения");
        code=integer(data,"version_code");version=data.getString("version_name");
        if(code<=installedCode||code>Integer.MAX_VALUE) throw new IOException("Установлена актуальная или более новая версия приложения");
        if(version.isEmpty()||version.length()>80||version.indexOf('\0')>=0) throw new IOException("Неверная версия приложения");
        if(!selectedAbi.equals("arm64-v8a")&&!selectedAbi.equals("armeabi-v7a")&&!selectedAbi.equals("x86_64")) throw new IOException("Архитектура не поддерживается");
        abi=selectedAbi;JSONObject file=data.getJSONObject("apks").getJSONObject(abi);
        bytes=integer(file,"bytes");sha256=file.getString("sha256");url=file.getString("url");
        AppUpdatePolicy.https(url);
        if(!url.startsWith("https://github.com/AnyaSheriff/MCSheriffAnya/releases/download/")||!new java.net.URI(url).getPath().endsWith(".apk")||bytes<1_000_000||bytes>AppUpdatePolicy.MAX_APK||!sha256.matches("[0-9a-f]{64}")) throw new IOException("Неверные параметры APK");
        raw=input.clone();
    }
    private static long integer(JSONObject json,String key) throws Exception {
        Object value=json.get(key);if(!(value instanceof Integer)&&!(value instanceof Long)) throw new IOException("Неверное целое число в описании APK");return ((Number)value).longValue();
    }
    static AppUpdateManifest parse(byte[] input,String abi,long installedCode) throws IOException {
        if(input.length==0||input.length>65536) throw new IOException("Неверный размер описания приложения");
        try { return new AppUpdateManifest(input,abi,installedCode); }
        catch(IOException error) { throw error; }catch(Exception error) { throw new IOException("Некорректное описание обновления приложения",error); }
    }
}
