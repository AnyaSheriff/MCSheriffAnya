package ru.sheriffanya.mcsa;

import android.util.Base64;
import org.json.JSONObject;
import java.io.IOException;
import java.net.URI;
import java.nio.charset.StandardCharsets;

/** Подпись проверяется над исходными байтами payload, до доверия к полям JSON. */
final class CoreManifest {
    static final long MAX_MODULE=128L*1024*1024;
    final byte[] envelope;final int revision,minApp,hostApi;final String version,source,buildId,notes,abi,url,sha256;final long bytes;
    private CoreManifest(byte[] raw,JSONObject data,String selectedAbi,int appVersion) throws Exception {
        if(data.getInt("schema")!=1||!"preview".equals(data.getString("channel"))||!"26.3".equals(data.getString("game"))||data.getInt("data_compat")!=1) throw new IOException("Эта версия ядра требует другого приложения");
        revision=data.getInt("revision");minApp=data.getInt("min_app");hostApi=data.getInt("host_api");
        if(revision<1||minApp<1||hostApi!=1||minApp>appVersion) throw new IOException("Для этого ядра нужно обновить приложение");
        version=text(data,"version",80);source=data.getString("source");buildId=data.getString("build_id");notes=text(data,"notes",2000);
        if(!source.matches("[0-9a-f]{40}")||!buildId.matches("[0-9a-f]{40}-r[1-9][0-9]{0,8}")||!buildId.startsWith(source+"-")) throw new IOException("Неверный идентификатор сборки ядра");
        if(!selectedAbi.equals("arm64-v8a")&&!selectedAbi.equals("armeabi-v7a")&&!selectedAbi.equals("x86_64")) throw new IOException("Архитектура устройства не поддерживается");
        JSONObject file=data.getJSONObject("packages").getJSONObject(selectedAbi);abi=selectedAbi;url=file.getString("url");https(url);bytes=file.getLong("bytes");sha256=file.getString("sha256");
        if(bytes<1_000_000||bytes>MAX_MODULE||!sha256.matches("[0-9a-f]{64}")) throw new IOException("Неверные параметры файла ядра");
        envelope=raw.clone();
    }
    static CoreManifest parse(byte[] raw,byte[] publicKey,String abi,int appVersion) throws IOException {
        if(raw.length==0||raw.length>65536) throw new IOException("Неверный размер описания обновления");
        try {
            JSONObject outer=new JSONObject(new String(raw,StandardCharsets.UTF_8));
            byte[] payload=Base64.decode(outer.getString("payload"),Base64.DEFAULT),signature=Base64.decode(outer.getString("signature"),Base64.DEFAULT);
            if(!CoreSignature.verify(publicKey,payload,signature)) throw new IOException("Подпись обновления не подтверждена");
            return new CoreManifest(raw,new JSONObject(new String(payload,StandardCharsets.UTF_8)),abi,appVersion);
        } catch(IOException invalid) { throw invalid; }
        catch(Exception invalid) { throw new IOException("Некорректное описание обновления",invalid); }
    }
    static URI https(String value) throws IOException {
        try {
            URI uri=new URI(value);
            if(!"https".equalsIgnoreCase(uri.getScheme())||uri.getHost()==null||uri.getUserInfo()!=null||uri.getFragment()!=null||(uri.getPort()!=-1&&uri.getPort()!=443)) throw new IOException("Обновление должно загружаться по HTTPS");
            return uri;
        } catch(java.net.URISyntaxException invalid) { throw new IOException("Неверный адрес обновления",invalid); }
    }
    private static String text(JSONObject data,String key,int limit) throws Exception {
        String value=data.getString(key);if(value.isEmpty()||value.length()>limit||value.indexOf('\0')>=0) throw new IOException("Неверное поле обновления");return value;
    }
}
