package ru.sheriffanya.launcher;

import java.io.*;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.zip.*;

/** Автономные проверки: сеть и настоящий сервер здесь не используются. */
final class LauncherSelfTest {
    private static int checks;
    private LauncherSelfTest() {}

    static void run() throws Exception {
        checks = 0;
        tarTests();
        propertiesTests();
        jsonAndUrlTests();
        System.out.println("Самопроверка: " + checks + " проверок пройдено, сеть не использовалась.");
    }

    private static void check(boolean ok, String name) {
        if (!ok) throw new AssertionError(name);
        checks++;
    }

    @FunctionalInterface private interface Action { void run() throws Exception; }

    private static void rejects(Action action, String name) throws Exception {
        try { action.run(); }
        catch (IOException e) { checks++; return; }
        throw new AssertionError("Ожидался отказ: " + name);
    }

    private static void tarTests() throws Exception {
        byte[] binary = new byte[1237];
        new Random(7).nextBytes(binary);
        ByteArrayOutputStream tar = new ByteArrayOutputStream();
        entry(tar, "release/", "", '5', new byte[0]);
        entry(tar, "release/README.md", "", '0', "текст\n".getBytes(StandardCharsets.UTF_8));
        entry(tar, "../../outside", "", '0', new byte[]{1, 2, 3});
        entry(tar, "mcsheriffanya", "release", '0', binary);
        entry(tar, "release/LICENSE", "", '0', new byte[513]);
        tar.write(new byte[1024]);
        check(Arrays.equals(unpack(gzip(tar.toByteArray())), binary), "ustar prefix, пропуск файлов и padding");

        ByteArrayOutputStream root = new ByteArrayOutputStream();
        entry(root, "./mcsheriffanya", "", '0', binary);
        root.write(new byte[1024]);
        check(Arrays.equals(unpack(gzip(root.toByteArray())), binary), "бинарник в корне");
        byte[] corrupt = tar.toByteArray();
        corrupt[0] ^= 1;
        rejects(() -> unpack(gzip(corrupt)), "checksum заголовка");
        byte[] compressed = gzip(tar.toByteArray());
        compressed[compressed.length - 8] ^= 1;
        rejects(() -> unpack(compressed), "CRC gzip");
        rejects(() -> unpack(gzip(Arrays.copyOf(root.toByteArray(), 700))), "оборванные данные");
        rejects(() -> unpack(gzip(Arrays.copyOf(root.toByteArray(), 2048))), "нет конца tar");
        rejects(() -> unpack(gzip(new byte[1024])), "нет бинарника");
        for (char type : new char[]{'1', '2', '5'}) {
            ByteArrayOutputStream wrong = new ByteArrayOutputStream();
            entry(wrong, "release/mcsheriffanya", "", type, binary);
            wrong.write(new byte[1024]);
            rejects(() -> unpack(gzip(wrong.toByteArray())), "ссылка или каталог вместо файла");
        }
        ByteArrayOutputStream empty = new ByteArrayOutputStream();
        entry(empty, "mcsheriffanya", "", '0', new byte[0]);
        empty.write(new byte[1024]);
        rejects(() -> unpack(gzip(empty.toByteArray())), "пустой бинарник");
        ByteArrayOutputStream duplicate = new ByteArrayOutputStream();
        entry(duplicate, "mcsheriffanya", "", '0', binary);
        entry(duplicate, "release/mcsheriffanya", "", '0', binary);
        duplicate.write(new byte[1024]);
        rejects(() -> unpack(gzip(duplicate.toByteArray())), "два бинарника");
        ByteArrayOutputStream traversal = new ByteArrayOutputStream();
        entry(traversal, "../mcsheriffanya", "", '0', binary);
        traversal.write(new byte[1024]);
        rejects(() -> unpack(gzip(traversal.toByteArray())), "путь вне каталога");
    }

    private static byte[] unpack(byte[] gzip) throws IOException {
        ByteArrayOutputStream result = new ByteArrayOutputStream();
        try (InputStream input = new GZIPInputStream(new ByteArrayInputStream(gzip))) {
            Launcher.extractTar(input, result, "release");
        }
        return result.toByteArray();
    }

    private static byte[] gzip(byte[] bytes) throws IOException {
        ByteArrayOutputStream result = new ByteArrayOutputStream();
        try (GZIPOutputStream output = new GZIPOutputStream(result)) { output.write(bytes); }
        return result.toByteArray();
    }

    private static void entry(OutputStream out, String name, String prefix, char type, byte[] data) throws IOException {
        byte[] header = new byte[512];
        put(header, 0, name);
        put(header, 100, "0000755");
        put(header, 124, String.format(Locale.ROOT, "%011o", data.length));
        Arrays.fill(header, 148, 156, (byte) ' ');
        header[156] = (byte) type;
        put(header, 257, "ustar");
        put(header, 263, "00");
        put(header, 345, prefix);
        long sum = 0;
        for (byte b : header) sum += b & 255;
        put(header, 148, String.format(Locale.ROOT, "%06o\0 ", sum));
        out.write(header);
        out.write(data);
        out.write(new byte[(512 - data.length % 512) % 512]);
    }

    private static void put(byte[] bytes, int offset, String text) {
        byte[] encoded = text.getBytes(StandardCharsets.UTF_8);
        System.arraycopy(encoded, 0, bytes, offset, encoded.length);
    }

    private static void propertiesTests() throws Exception {
        check(Launcher.choosePort(null, "25951").equals("25951"), "порт из -Dspt");
        check(Launcher.choosePort("25601", "25951").equals("25601"), "SERVER_PORT важнее -Dspt");
        check(Launcher.choosePort("25601", "bad").equals("25601"), "неиспользуемый -Dspt игнорируется");
        check(Launcher.choosePort(" ", " 25951 ").equals("25951"), "пробелы в портах панели");
        check(Launcher.choosePort(null, null) == null, "нет настроек порта");
        check(Launcher.choosePort(null, " ") == null, "пустой -Dspt");
        rejects(() -> Launcher.choosePort(null, "65536"), "неверный -Dspt");
        rejects(() -> Launcher.choosePort("bad", "25951"), "неверный SERVER_PORT не заменяется -Dspt");
        String original = "# пометка\r\n motd = Наш сервер\r\nserver-port=1\r\n"
                + "server-port=2\r\nonline-mode=\r\ncustom = a=b\r\nlast-line";
        Map<String, String> forced = new LinkedHashMap<>();
        forced.put("server-port", "25601");
        forced.put("server-ip", "0.0.0.0");
        String merged = Launcher.merge(original, forced, Map.of("motd", "панель", "online-mode", "true"));
        check(merged.contains("# пометка\r\n motd = Наш сервер\r\n"), "комментарии, русский текст, CRLF");
        check(merged.contains("online-mode=\r\ncustom = a=b\r\nlast-line\n"), "остальные строки сохранены");
        check(merged.indexOf("server-port=") == merged.lastIndexOf("server-port="), "повторения порта удалены");
        check(Launcher.properties(merged).get("server-port").equals("25601"), "порт панели");
        check(Launcher.properties(merged).get("server-ip").equals("0.0.0.0"), "адрес всех интерфейсов");
        check(Launcher.merge(merged, forced, Map.of()).equals(merged), "повторный запуск не меняет файл");
        for (String port : List.of("1", "25601", "65535")) { Launcher.validatePort(port); checks++; }
        Launcher.validatePort(null);
        for (String port : List.of("0", "65536", "-1", "25601\nkey=x", "abc", "", "+80")) {
            rejects(() -> Launcher.validatePort(port), "неверный порт");
        }
        Path cwd = Files.createTempDirectory("mcsa-launcher-self-test-");
        try {
            Launcher.configure(cwd, null);
            check(!Files.exists(cwd.resolve("config")), "без порта и корневого файла настройки не создаются");
            Files.writeString(cwd.resolve("server.properties"), "motd=Панель=тест\nmax-players=7\nonline-mode=false\nserver-port=12345\nsecret=ignored\n");
            Launcher.configure(cwd, "25601");
            Path server = cwd.resolve("config/server.properties");
            Path own = cwd.resolve("config/mcsa.properties");
            Map<String, String> props = Launcher.properties(Files.readString(server));
            check(props.get("motd").equals("Панель=тест") && props.get("max-players").equals("7")
                    && props.get("online-mode").equals("false") && !props.containsKey("secret"), "только три ключа панели");
            check(props.get("server-port").equals("25601"), "порт корневого файла не копируется");
            check(Files.readString(own).equals("bedrock-port=25601\n"), "TCP и UDP имеют один номер");
            Files.writeString(own, "# UDP выключен\nbedrock-port=0\ncustom=yes\n");
            String keep = Files.readString(own);
            Launcher.configure(cwd, "25602");
            check(Files.readString(own).equals(keep), "настроенный bedrock-port, включая 0, сохраняется");
            String before = Files.readString(server);
            Files.writeString(cwd.resolve("server.properties"), "motd=Новая панель\nmax-players=99\nonline-mode=true\n");
            Launcher.configure(cwd, null);
            check(Files.readString(server).equals(before), "свои настройки важнее панели, отсутствие SERVER_PORT");
            rejects(() -> Launcher.configure(cwd, "bad"), "плохой порт до записи");
            check(Files.readString(server).equals(before), "неверный порт не изменил файл");
        } finally {
            try (var files = Files.walk(cwd)) {
                for (Path path : files.sorted(Comparator.reverseOrder()).toList()) Files.delete(path);
            }
        }
    }

    private static void jsonAndUrlTests() throws Exception {
        String name = "mcsheriffanya-v0.2.3-alpha-x86_64-unknown-linux-musl.tar.gz";
        String json = "{\"body\":\"fake \\\"tag_name\\\": \\\"bad\\\" \\u0410\","
                + "\"unknown\":[null,true,false,1.5,1e3],\"tag_name\":\"v0.2.3-alpha\",\"assets\":["
                + "{\"name\":\"other\",\"size\":1},{\"name\":\"" + name + "\",\"size\":123,"
                + "\"browser_download_url\":\"https:\\/\\/github.com/a.tar.gz\"}]}";
        Launcher.Asset asset = Launcher.selectAsset(json, null);
        check(asset.name().equals(name) && asset.size() == 123, "JSON, escape и выбор архива");
        rejects(() -> Launcher.selectAsset(json, "v0.2.4-alpha"), "другая версия");
        rejects(() -> Launcher.selectAsset(json.replace(name, "missing"), null), "нет нужного архива");
        rejects(() -> new Launcher.Json("{\"x\":1,}").parse(), "плохой JSON");
        rejects(() -> new Launcher.Json("{} trailing").parse(), "лишний JSON");
        for (String url : List.of("http://github.com/a", "https://evil.example/a", "https://github.com.evil.example/a",
                "https://github.com@evil.example/a", "https://user@github.com/a", "https://github.com:444/a",
                "file:///tmp/a", "https://api.github.com/a", "https://github.com/a#fragment")) {
            rejects(() -> Launcher.validateUrl(URI.create(url), false), "запрет адреса");
        }
        Launcher.validateUrl(URI.create("https://github.com/a"), false);
        Launcher.validateUrl(URI.create("https://objects.githubusercontent.com/a"), false);
        Launcher.validateUrl(URI.create(Launcher.API + "latest"), true);
        checks += 3;
        for (String url : List.of("https://release-assets.githubusercontent.com/a",
                "https://RELEASE-ASSETS.GITHUBUSERCONTENT.COM:443/a?download=1")) {
            Launcher.validateUrl(URI.create(url), false);
            checks++;
        }
        for (String url : List.of("http://release-assets.githubusercontent.com/a",
                "https://release-assets.githubusercontent.com.evil.example/a",
                "https://evil.release-assets.githubusercontent.com/a",
                "https://release-assets.githubusercontent.com@evil.example/a",
                "https://user@release-assets.githubusercontent.com/a",
                "https://release-assets.githubusercontent.com:444/a",
                "https://release-assets.githubusercontent.com/a#fragment",
                "https://release-assets.githubusercontent.com./a",
                "https://other.githubusercontent.com/a")) {
            rejects(() -> Launcher.validateUrl(URI.create(url), false), "запрет похожих адресов и неверного HTTPS");
        }
        rejects(() -> Launcher.validateUrl(URI.create("https://release-assets.githubusercontent.com/a"), true),
                "адрес файлов не разрешён для API");
        rejects(() -> Launcher.validateUrl(URI.create("https://github.com/a"), true), "API только на api.github.com");
        rejects(() -> Launcher.copyLimited(new ByteArrayInputStream(new byte[11]), OutputStream.nullOutputStream(), 10), "лимит загрузки");
    }
}
