package ru.sheriffanya.launcher;

import java.io.*;
import java.net.*;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.TimeUnit;
import java.util.zip.GZIPInputStream;

/** Маленький переходник между Java-панелью хостинга и родным сервером. */
public final class Launcher {
    static final String VERSION = "1.0.2";
    static final String API = "https://api.github.com/repos/AnyaSheriff/MCSheriffAnya/releases/";
    static final long ARCHIVE_LIMIT = 128L * 1024 * 1024;
    static final long TAR_LIMIT = 512L * 1024 * 1024;
    static final long BINARY_LIMIT = 256L * 1024 * 1024;

    private Launcher() {}

    public static void main(String[] args) {
        System.out.println("MCSheriffAnya launcher " + VERSION);
        int code;
        try {
            if (args.length == 1 && args[0].equals("--launcher-self-test")) {
                LauncherSelfTest.run();
                return;
            }
            code = run(Path.of("").toAbsolutePath().normalize(), args);
        } catch (LauncherFailure e) {
            System.err.println("Ошибка launcher: " + e.getMessage());
            code = 1;
        } catch (Exception e) {
            // Не выводим URL, аргументы, окружение и чужие сообщения исключений:
            // в них могут оказаться пароли или подписанные ссылки GitHub.
            System.err.println("Ошибка launcher: операция не выполнена ("
                    + e.getClass().getSimpleName() + "). Проверьте сеть и права на папку сервера.");
            code = 1;
        }
        System.exit(code);
    }

    static int run(Path cwd, String[] args) throws Exception {
        String port = choosePort(setting("SERVER_PORT"), System.getProperty("spt"));
        Path binary = cwd.resolve("mcsheriffanya");
        Path marker = cwd.resolve("mcsa-update");
        prepareBinary(cwd, setting("MCSA_VERSION"), setting("MCSA_AUTO_UPDATE"),
                Files.exists(marker) || "1".equals(setting("MCSA_UPDATE")), new Updates() {
                    public Asset release(String tag) throws IOException { return fetchRelease(tag); }
                    public void install(Asset asset) throws IOException {
                        Launcher.install(cwd, binary, asset, uri -> open(uri, false));
                    }
                });
        if (!Files.isRegularFile(binary) || Files.size(binary) == 0) {
            throw failure("Файл mcsheriffanya пуст или не является обычным файлом.");
        }
        if (!binary.toFile().setExecutable(true) && !Files.isExecutable(binary)) {
            throw permissionFailure();
        }
        configure(cwd, port);
        List<String> command = new ArrayList<>();
        command.add(binary.toString());
        command.addAll(Arrays.asList(args));

        // Общий замок закрывает промежуток между запуском и установкой обработчика.
        Object lock = new Object();
        Process[] child = new Process[1];
        boolean[] stopping = {false};
        Thread hook = new Thread(() -> {
            Process process;
            synchronized (lock) {
                stopping[0] = true;
                process = child[0];
            }
            if (process == null || !process.isAlive()) return;
            System.out.println("Остановка сервера: передаём SIGTERM, ждём сохранения мира до 30 секунд.");
            process.destroy();
            try {
                if (!process.waitFor(30, TimeUnit.SECONDS)) {
                    System.err.println("Сервер не завершился за 30 секунд; принудительная остановка.");
                    process.destroyForcibly();
                }
            } catch (InterruptedException e) {
                process.destroyForcibly();
                Thread.currentThread().interrupt();
            }
        }, "mcsa-shutdown");
        Runtime.getRuntime().addShutdownHook(hook);
        synchronized (lock) {
            if (stopping[0]) return 1;
            System.out.println("Запускаем mcsheriffanya. Консоль панели работает в построчном режиме.");
            try {
                // Панели часто предоставляют PTY: inheritIO включал в ядре
                // посимвольную перерисовку "> команда", которую панель разбивала
                // на отдельные строки. Канал stdin выбирает обычный read_lines
                // и у уже выпущенных версий ядра; stdout/stderr остаются прямыми.
                child[0] = new ProcessBuilder(command).directory(cwd.toFile())
                        .redirectOutput(ProcessBuilder.Redirect.INHERIT)
                        .redirectError(ProcessBuilder.Redirect.INHERIT).start();
                forwardInput(child[0]);
            } catch (IOException e) {
                String message = String.valueOf(e.getMessage()).toLowerCase(Locale.ROOT);
                if (message.contains("error=13") || message.contains("permission denied")) {
                    throw permissionFailure();
                }
                throw failure("Не удалось запустить mcsheriffanya. Нужен Linux x86_64 и подходящий исполняемый файл.");
            }
        }
        // Обработчик остаётся до выхода JVM: после обычного stop ребёнок уже завершён.
        return child[0].waitFor();
    }

    static void forwardInput(Process process) {
        Thread input = new Thread(() -> {
            try (OutputStream output = process.getOutputStream()) {
                byte[] bytes = new byte[8192];
                int count;
                while ((count = System.in.read(bytes)) != -1) {
                    output.write(bytes, 0, count);
                    output.flush();
                }
            } catch (IOException e) {
                if (process.isAlive()) {
                    System.err.println("Не удалось передать ввод консоли серверу.");
                }
            }
        }, "mcsa-console-input");
        // Чтение stdin может ждать бесконечно: завершившийся сервер не должен
        // удерживать JVM из-за открытого терминала панели.
        input.setDaemon(true);
        input.start();
    }

    static LauncherFailure permissionFailure() {
        return failure("Хостинг запрещает запуск программ из папки сервера (noexec или права доступа). "
                + "Попросите поддержку разрешить запуск mcsheriffanya; один server.jar не обходит этот запрет.");
    }

    static String setting(String name) {
        String value = System.getProperty(name);
        if (value == null) value = System.getenv(name);
        return value == null || value.isBlank() ? null : value.trim();
    }

    interface Updates {
        Asset release(String tag) throws IOException;
        void install(Asset asset) throws IOException;
    }

    @FunctionalInterface interface Download { InputStream open(URI uri) throws IOException; }

    static void prepareBinary(Path cwd, String pinned, String autoUpdate, boolean manual,
            Updates updates) throws IOException {
        Path binary = cwd.resolve("mcsheriffanya");
        boolean installed = Files.isRegularFile(binary) && Files.size(binary) > 0;
        if (installed && !manual && (pinned != null || "0".equals(autoUpdate))) return;
        try {
            Asset asset = updates.release(pinned);
            Path version = cwd.resolve("mcsheriffanya.version");
            String current = readOptional(version).trim();
            if (!installed || manual || current.isEmpty() || compareVersions(asset.tag(), current) > 0) {
                updates.install(asset);
                writeChanged(version, readOptional(version), asset.tag() + "\n");
            }
            if (manual) Files.deleteIfExists(cwd.resolve("mcsa-update"));
        } catch (IOException | RuntimeException e) {
            if (!installed) throw e;
            // Не печатаем чужие сообщения исключений или подписанные URL.
            System.err.println("Не удалось проверить или установить обновление; запускаем установленный mcsheriffanya.");
        }
    }

    // Числовые компоненты и prerelease по SemVer; v и build metadata не влияют.
    static int compareVersions(String left, String right) throws IOException {
        String[] a = versionParts(left), b = versionParts(right);
        String[] ac = a[0].split("\\."), bc = b[0].split("\\.");
        for (int i = 0; i < Math.max(ac.length, bc.length); i++) {
            int order = new BigInteger(i < ac.length ? ac[i] : "0")
                    .compareTo(new BigInteger(i < bc.length ? bc[i] : "0"));
            if (order != 0) return order;
        }
        if (a.length == 1 || b.length == 1) return Integer.compare(b.length, a.length);
        String[] ap = a[1].split("\\."), bp = b[1].split("\\.");
        for (int i = 0; i < Math.min(ap.length, bp.length); i++) {
            boolean an = ap[i].matches("[0-9]+"), bn = bp[i].matches("[0-9]+");
            int order = an && bn ? new BigInteger(ap[i]).compareTo(new BigInteger(bp[i]))
                    : an != bn ? (an ? -1 : 1) : ap[i].compareTo(bp[i]);
            if (order != 0) return order;
        }
        return Integer.compare(ap.length, bp.length);
    }

    static String[] versionParts(String tag) throws IOException {
        if (!tag.matches("v?[0-9]+(?:\\.[0-9]+)*(?:-[0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*)?"
                + "(?:\\+[0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*)?")) {
            throw failure("Не удалось сравнить метки выпусков.");
        }
        return tag.replaceFirst("^v", "").split("\\+", 2)[0].split("-", 2);
    }

    static Asset fetchRelease(String tag) throws IOException {
        if (tag != null && !tag.matches("[A-Za-z0-9][A-Za-z0-9._-]{0,99}")) {
            throw failure("MCSA_VERSION должен содержать метку выпуска, например v0.2.3-alpha.");
        }
        System.out.println("Ищем " + (tag == null ? "последний выпуск" : "указанный выпуск") + " на GitHub…");
        byte[] metadata;
        try (InputStream input = open(URI.create(API + (tag == null ? "latest" : "tags/" + tag)), true)) {
            ByteArrayOutputStream output = new ByteArrayOutputStream();
            copyLimited(input, output, 4 * 1024 * 1024);
            metadata = output.toByteArray();
        }
        return selectAsset(new String(metadata, StandardCharsets.UTF_8), tag);
    }

    static void install(Path cwd, Path binary, Asset asset, Download download) throws IOException {
        String arch = System.getProperty("os.arch", "");
        if (!System.getProperty("os.name", "").equals("Linux")
                || !(arch.equals("amd64") || arch.equals("x86_64"))) {
            throw failure("Автозагрузка поддерживает только Linux x86_64. Загрузите подходящий mcsheriffanya вручную.");
        }
        System.out.println("Скачиваем выпуск " + asset.tag() + " для Linux x86_64…");
        Path archive = Files.createTempFile(cwd, ".mcsa-download-", ".tar.gz");
        Path extracted = null;
        try {
            try (InputStream input = download.open(asset.url());
                    OutputStream output = Files.newOutputStream(archive)) {
                long count = copyLimited(input, output, ARCHIVE_LIMIT);
                if (count == 0 || count != asset.size()) throw failure("Размер архива не совпал с данными GitHub.");
            }
            System.out.println("Распаковываем только mcsheriffanya…");
            extracted = Files.createTempFile(cwd, ".mcsa-binary-", ".tmp");
            try (InputStream input = new GZIPInputStream(Files.newInputStream(archive));
                    OutputStream output = Files.newOutputStream(extracted)) {
                extractTar(input, output, asset.name().substring(0, asset.name().length() - 7));
            }
            if (Files.size(extracted) == 0) throw failure("В архиве пустой mcsheriffanya.");
            if (!extracted.toFile().setExecutable(true)) throw permissionFailure();
            atomicReplace(extracted, binary);
            System.out.println("Установлен выпуск " + asset.tag() + ".");
        } finally {
            Files.deleteIfExists(archive);
            if (extracted != null) Files.deleteIfExists(extracted);
        }
    }

    record Asset(String tag, String name, URI url, long size) {}

    static Asset selectAsset(String json, String requested) throws IOException {
        Object parsed = new Json(json).parse();
        if (!(parsed instanceof Map<?, ?> release)
                || !(release.get("tag_name") instanceof String tag)
                || !tag.matches("[A-Za-z0-9][A-Za-z0-9._-]{0,99}")
                || (requested != null && !requested.equals(tag))
                || !(release.get("assets") instanceof List<?> assets)) {
            throw failure("GitHub вернул непонятное описание выпуска.");
        }
        String name = "mcsheriffanya-" + tag + "-x86_64-unknown-linux-musl.tar.gz";
        for (Object item : assets) {
            if (item instanceof Map<?, ?> asset && name.equals(asset.get("name"))
                    && asset.get("browser_download_url") instanceof String url
                    && asset.get("size") instanceof Long size && size > 0 && size <= ARCHIVE_LIMIT) {
                URI uri;
                try { uri = URI.create(url); }
                catch (IllegalArgumentException e) { throw failure("GitHub вернул неверный адрес архива."); }
                validateUrl(uri, false);
                return new Asset(tag, name, uri, size);
            }
        }
        throw failure("В выпуске нет архива Linux musl x86_64. Укажите MCSA_VERSION с готовым выпуском "
                + "или загрузите mcsheriffanya вручную.");
    }

    static void validateUrl(URI uri, boolean api) throws IOException {
        String host = uri.getHost();
        boolean allowed = api ? "api.github.com".equalsIgnoreCase(host)
                : "github.com".equalsIgnoreCase(host) || "objects.githubusercontent.com".equalsIgnoreCase(host)
                || "release-assets.githubusercontent.com".equalsIgnoreCase(host);
        if (!"https".equalsIgnoreCase(uri.getScheme()) || !allowed || uri.getRawUserInfo() != null
                || (uri.getPort() != -1 && uri.getPort() != 443) || uri.getRawFragment() != null) {
            throw failure("Адрес скачивания запрещён: разрешены только указанные адреса GitHub по HTTPS.");
        }
    }

    static InputStream open(URI initial, boolean api) throws IOException {
        URI uri = initial;
        for (int redirects = 0; redirects <= 5; redirects++) {
            validateUrl(uri, api);
            HttpURLConnection connection = (HttpURLConnection) uri.toURL().openConnection();
            connection.setInstanceFollowRedirects(false);
            connection.setConnectTimeout(10_000);
            connection.setReadTimeout(10_000);
            connection.setRequestProperty("User-Agent", "MCSheriffAnya-launcher/" + VERSION);
            connection.setRequestProperty("Accept", api ? "application/vnd.github+json" : "application/octet-stream");
            try {
                int status = connection.getResponseCode();
                if (status == 301 || status == 302 || status == 303 || status == 307 || status == 308) {
                    String location = connection.getHeaderField("Location");
                    if (location == null) throw failure("GitHub не указал адрес перенаправления.");
                    try { uri = uri.resolve(location); }
                    catch (IllegalArgumentException e) { throw failure("GitHub вернул неверное перенаправление."); }
                    connection.disconnect();
                    continue;
                }
                if (status != 200) throw failure("GitHub ответил HTTP " + status
                        + ". Проверьте метку выпуска и доступность GitHub; возможен лимит запросов.");
                return new FilterInputStream(connection.getInputStream()) {
                    @Override public void close() throws IOException {
                        try { super.close(); } finally { connection.disconnect(); }
                    }
                };
            } catch (IOException | RuntimeException e) {
                connection.disconnect();
                throw e;
            }
        }
        throw failure("Слишком много перенаправлений GitHub.");
    }

    static long copyLimited(InputStream input, OutputStream output, long limit) throws IOException {
        byte[] buffer = new byte[16 * 1024];
        long count = 0;
        int n;
        while ((n = input.read(buffer)) != -1) {
            count += n;
            if (count > limit) throw failure("Файл превышает допустимый размер.");
            output.write(buffer, 0, n);
        }
        return count;
    }

    // ustar: читаем заголовки и данные, имена из архива никогда не становятся путями записи.
    static void extractTar(InputStream input, OutputStream output, String directory) throws IOException {
        boolean found = false;
        long total = 0;
        while (true) {
            byte[] header = input.readNBytes(512);
            if (header.length != 512) throw failure("Архив tar оборван.");
            total += 512;
            if (total > TAR_LIMIT) throw failure("Архив tar слишком большой.");
            if (zero(header)) {
                byte[] second = input.readNBytes(512);
                if (second.length != 512 || !zero(second)) throw failure("Неверный конец архива tar.");
                // Дочитываем gzip до конца: проверяется CRC, мусор после tar не принимается.
                byte[] padding = new byte[8192];
                int n;
                while ((n = input.read(padding)) != -1) {
                    total += n;
                    if (total > TAR_LIMIT) throw failure("Архив tar слишком большой.");
                    for (int i = 0; i < n; i++) if (padding[i] != 0) throw failure("Лишние данные после tar.");
                }
                if (!found) throw failure("В архиве не найден mcsheriffanya.");
                return;
            }
            long sum = 0;
            for (int i = 0; i < 512; i++) sum += i >= 148 && i < 156 ? 32 : header[i] & 255;
            if (sum != octal(header, 148, 8) || !field(header, 257, 6).startsWith("ustar")) {
                throw failure("Повреждён заголовок ustar.");
            }
            String name = field(header, 0, 100);
            String prefix = field(header, 345, 155);
            if (!prefix.isEmpty()) name = prefix + "/" + name;
            if (name.startsWith("./")) name = name.substring(2);
            long size = octal(header, 124, 12);
            long padding = (512 - size % 512) % 512;
            if (size > TAR_LIMIT - total - padding) throw failure("Архив tar слишком большой.");
            total += size + padding;
            boolean wanted = name.equals("mcsheriffanya") || name.equals(directory + "/mcsheriffanya");
            if (wanted) {
                if (found || (header[156] != 0 && header[156] != '0') || size == 0 || size > BINARY_LIMIT) {
                    throw failure("Неверный или повторный файл mcsheriffanya в архиве.");
                }
                byte[] buffer = new byte[16 * 1024];
                long remaining = size;
                while (remaining > 0) {
                    int n = input.read(buffer, 0, (int) Math.min(buffer.length, remaining));
                    if (n == -1) throw failure("Файл в tar оборван.");
                    output.write(buffer, 0, n);
                    remaining -= n;
                }
                found = true;
            } else {
                input.skipNBytes(size);
            }
            input.skipNBytes(padding);
        }
    }

    static boolean zero(byte[] bytes) {
        for (byte b : bytes) if (b != 0) return false;
        return true;
    }

    static String field(byte[] bytes, int start, int length) {
        int end = start;
        while (end < start + length && bytes[end] != 0) end++;
        return new String(bytes, start, end - start, StandardCharsets.UTF_8);
    }

    static long octal(byte[] bytes, int start, int length) throws IOException {
        String value = field(bytes, start, length).trim();
        if (!value.matches("[0-7]+")) throw failure("Неверное число в tar.");
        try { return Long.parseLong(value, 8); }
        catch (NumberFormatException e) { throw failure("Слишком большое число в tar."); }
    }

    // Hosting-Minecraft.pro передаёт выделенный порт параметром JVM -Dspt.
    static String choosePort(String configured, String panel) throws IOException {
        String port = configured;
        if (port == null || port.isBlank()) port = panel;
        if (port != null) port = port.isBlank() ? null : port.trim();
        validatePort(port);
        return port;
    }

    static void validatePort(String port) throws IOException {
        if (port == null) return;
        if (!port.matches("[0-9]{1,5}") || Integer.parseInt(port) < 1 || Integer.parseInt(port) > 65535) {
            throw failure("Порт (SERVER_PORT или -Dspt) должен быть целым числом от 1 до 65535.");
        }
    }

    // Тот же простой key=value, что src/config/server_properties.rs: без Java-экранирования.
    static Map<String, String> properties(String text) {
        Map<String, String> result = new LinkedHashMap<>();
        for (String line : text.split("\n")) {
            String trimmed = line.trim();
            int equals = trimmed.indexOf('=');
            if (!trimmed.startsWith("#") && equals >= 0) {
                result.put(trimmed.substring(0, equals).trim(), trimmed.substring(equals + 1).trim());
            }
        }
        return result;
    }

    static String merge(String original, Map<String, String> forced, Map<String, String> defaults) {
        Map<String, String> pending = new LinkedHashMap<>(forced);
        Map<String, String> existing = properties(original);
        defaults.forEach((key, value) -> { if (!existing.containsKey(key)) pending.putIfAbsent(key, value); });
        StringBuilder result = new StringBuilder();
        for (String line : original.split("(?<=\n)", -1)) {
            Map<String, String> parsed = properties(line);
            String key = parsed.isEmpty() ? null : parsed.keySet().iterator().next();
            if (key != null && forced.containsKey(key)) {
                // Удаляем повторения управляемого ключа: сервер берёт последнее значение.
                if (pending.containsKey(key)) {
                    String ending = line.endsWith("\r\n") ? "\r\n" : line.endsWith("\n") ? "\n" : "";
                    result.append(key).append('=').append(pending.remove(key)).append(ending);
                }
            } else result.append(line);
        }
        pending.forEach((key, value) -> {
            if (result.length() > 0 && result.charAt(result.length() - 1) != '\n') result.append('\n');
            result.append(key).append('=').append(value).append('\n');
        });
        return result.toString();
    }

    static void configure(Path cwd, String port) throws IOException {
        validatePort(port);
        Path config = cwd.resolve("config");
        Path server = config.resolve("server.properties");
        String original = readOptional(server);
        Map<String, String> defaults = new LinkedHashMap<>();
        Map<String, String> root = properties(readOptional(cwd.resolve("server.properties")));
        for (String key : List.of("motd", "max-players", "online-mode")) {
            if (root.containsKey(key)) defaults.put(key, root.get(key));
        }
        Map<String, String> forced = new LinkedHashMap<>();
        if (port != null) {
            forced.put("server-port", port);
            forced.put("server-ip", "0.0.0.0");
        }
        writeChanged(server, original, merge(original, forced, defaults));
        if (port != null) {
            Path own = config.resolve("mcsa.properties");
            String ownText = readOptional(own);
            writeChanged(own, ownText, merge(ownText, Map.of(), Map.of("bedrock-port", port)));
            System.out.println("Порт Java: " + port + " (TCP). Настройки Bedrock: config/mcsa.properties.");
        }
    }

    static String readOptional(Path file) throws IOException {
        return Files.exists(file) ? Files.readString(file, StandardCharsets.UTF_8) : "";
    }

    static void writeChanged(Path file, String old, String content) throws IOException {
        if (old.equals(content)) return;
        Files.createDirectories(file.getParent());
        Path temp = Files.createTempFile(file.getParent(), ".mcsa-config-", ".tmp");
        try {
            Files.writeString(temp, content, StandardCharsets.UTF_8);
            atomicReplace(temp, file);
        } finally { Files.deleteIfExists(temp); }
    }

    static void atomicReplace(Path source, Path target) throws IOException {
        try { Files.move(source, target, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING); }
        catch (AtomicMoveNotSupportedException e) {
            throw failure("Папка сервера не поддерживает безопасную атомарную замену файла; прежний файл оставлен.");
        }
    }

    static LauncherFailure failure(String message) { return new LauncherFailure(message); }

    static final class LauncherFailure extends IOException {
        private static final long serialVersionUID = 1L;
        LauncherFailure(String message) { super(message); }
    }

    /** Минимальный JSON: строки с escape, объекты, массивы и скалярные значения. */
    static final class Json {
        private final String text;
        private int pos;
        Json(String text) { this.text = text; }

        Object parse() throws IOException {
            Object value = value(0);
            whitespace();
            if (pos != text.length()) throw failure("Лишние данные в JSON GitHub.");
            return value;
        }

        private void whitespace() {
            while (pos < text.length() && " \t\r\n".indexOf(text.charAt(pos)) >= 0) pos++;
        }

        private boolean take(char c) {
            whitespace();
            if (pos < text.length() && text.charAt(pos) == c) { pos++; return true; }
            return false;
        }

        private void require(char c) throws IOException {
            if (!take(c)) throw failure("Неверный JSON GitHub.");
        }

        private Object value(int depth) throws IOException {
            whitespace();
            if (depth > 64 || pos == text.length()) throw failure("Неверный JSON GitHub.");
            char c = text.charAt(pos);
            if (c == '"') return string();
            if (take('{')) {
                Map<String, Object> map = new LinkedHashMap<>();
                if (take('}')) return map;
                do {
                    String key = string();
                    require(':');
                    map.put(key, value(depth + 1));
                } while (take(','));
                require('}');
                return map;
            }
            if (take('[')) {
                List<Object> list = new ArrayList<>();
                if (take(']')) return list;
                do { list.add(value(depth + 1)); } while (take(','));
                require(']');
                return list;
            }
            for (String literal : List.of("true", "false", "null")) {
                if (text.startsWith(literal, pos)) {
                    pos += literal.length();
                    return literal.equals("null") ? null : Boolean.valueOf(literal);
                }
            }
            int start = pos;
            while (pos < text.length() && "-+0123456789.eE".indexOf(text.charAt(pos)) >= 0) pos++;
            String number = text.substring(start, pos);
            if (!number.matches("-?(0|[1-9][0-9]*)(\\.[0-9]+)?([eE][+-]?[0-9]+)?")) throw failure("Неверное число JSON.");
            try {
                if (!number.contains(".") && !number.contains("e") && !number.contains("E")) return Long.valueOf(number);
                return Double.valueOf(number);
            } catch (NumberFormatException e) { throw failure("Слишком большое число JSON."); }
        }

        private String string() throws IOException {
            require('"');
            StringBuilder out = new StringBuilder();
            while (pos < text.length()) {
                char c = text.charAt(pos++);
                if (c == '"') return out.toString();
                if (c < 32) throw failure("Неверная строка JSON.");
                if (c != '\\') { out.append(c); continue; }
                if (pos == text.length()) break;
                c = text.charAt(pos++);
                switch (c) {
                    case '"', '\\', '/' -> out.append(c);
                    case 'b' -> out.append('\b');
                    case 'f' -> out.append('\f');
                    case 'n' -> out.append('\n');
                    case 'r' -> out.append('\r');
                    case 't' -> out.append('\t');
                    case 'u' -> {
                        if (pos + 4 > text.length()) throw failure("Оборванная строка JSON.");
                        try { out.append((char) Integer.parseInt(text.substring(pos, pos + 4), 16)); }
                        catch (NumberFormatException e) { throw failure("Неверный символ JSON."); }
                        pos += 4;
                    }
                    default -> throw failure("Неверный escape JSON.");
                }
            }
            throw failure("Оборванная строка JSON.");
        }
    }
}
