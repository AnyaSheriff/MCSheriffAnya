package ru.sheriffanya.mcsa;

import java.io.IOException;
import java.net.URI;
import java.util.Arrays;

/** Общие проверки до передачи APK системному установщику. */
final class AppUpdatePolicy {
    static final long MAX_APK=256L*1024*1024;
    static URI https(String address) throws IOException {
        try {
            URI uri=new URI(address);String host=uri.getHost();
            if(!"https".equals(uri.getScheme())||uri.getUserInfo()!=null||uri.getFragment()!=null||(uri.getPort()!=-1&&uri.getPort()!=443)
                    ||!("github.com".equals(host)||"raw.githubusercontent.com".equals(host)||"release-assets.githubusercontent.com".equals(host)||"objects.githubusercontent.com".equals(host))) throw new IOException("Недопустимый адрес обновления приложения");
            return uri;
        } catch(java.net.URISyntaxException error) { throw new IOException("Неверный адрес обновления",error); }
    }
    static void identity(String name,long code,String version,String expectedVersion,long expectedCode,String[] installed,String[] candidate) throws IOException {
        if(!"ru.sheriffanya.mcsa".equals(name)||code!=expectedCode||!version.equals(expectedVersion)) throw new IOException("APK не соответствует описанию обновления");
        if(installed==null||candidate==null||installed.length==0||candidate.length==0) throw new IOException("Не удалось проверить подпись приложения");
        String[] a=installed.clone(),b=candidate.clone();Arrays.sort(a);Arrays.sort(b);
        if(!Arrays.equals(a,b)) throw new IOException("APK подписан другим ключом");
    }
}
