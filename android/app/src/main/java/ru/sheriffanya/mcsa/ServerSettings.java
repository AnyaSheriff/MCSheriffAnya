package ru.sheriffanya.mcsa;

import android.util.AtomicFile;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

final class ServerSettings {
    static final class Field {
        final String group, key, title, help, fallback;
        final boolean mcsa;
        final int min, max;
        final String[] choices, labels;
        Field(String group, boolean mcsa, String key, String title, String help, String fallback, int min, int max, String[] choices, String[] labels) {
            this.group=group; this.mcsa=mcsa; this.key=key; this.title=title; this.help=help;
            this.fallback=fallback; this.min=min; this.max=max; this.choices=choices; this.labels=labels;
        }
        boolean toggle() { return choices != null && choices.length == 2 && choices[0].equals("false"); }
        void validate(String value) {
            if (value.length()>2048 || value.indexOf('\n')>=0 || value.indexOf('\r')>=0 || value.indexOf('\0')>=0)
                throw new IllegalArgumentException(title+": недопустимое значение");
            if (choices != null) {
                for (String choice:choices) if (choice.equals(value)) return;
                throw new IllegalArgumentException(title+": выбери значение из списка");
            }
            if (min != Integer.MIN_VALUE) {
                try { int n=Integer.parseInt(value); if (n>=min && n<=max) return; } catch (NumberFormatException ignored) { }
                throw new IllegalArgumentException(title+": число от "+min+" до "+max);
            }
        }
    }
    static final List<Field> FIELDS = new ArrayList<>();
    static void number(String group, boolean mcsa, String key, String title, String help, String fallback, int min, int max) {
        FIELDS.add(new Field(group,mcsa,key,title,help,fallback,min,max,null,null));
    }
    static void text(String group, boolean mcsa, String key, String title, String help, String fallback) {
        number(group,mcsa,key,title,help,fallback,Integer.MIN_VALUE,0);
    }
    static void choice(String group, boolean mcsa, String key, String title, String help, String fallback, String[] values, String[] labels) {
        FIELDS.add(new Field(group,mcsa,key,title,help,fallback,0,0,values,labels));
    }
    static void toggle(String group, boolean mcsa, String key, String title, String help, boolean value) {
        choice(group,mcsa,key,title,help,Boolean.toString(value),new String[]{"false","true"},new String[]{"Выключено","Включено"});
    }
    static {
        text("Сервер",false,"motd","Название сервера","Название в списке серверов Minecraft.","MCSheriffAnya • Android");
        number("Сервер",false,"server-port","Порт Java","TCP. Для Java-клиента; обычно 25565.","25565",1024,65535);
        number("Сервер",true,"bedrock-port","Порт Bedrock","UDP. 0 — отключить Bedrock. Частичная поддержка клиента остаётся экспериментальной.","19132",0,65535);
        number("Сервер",false,"max-players","Мест для игроков","Максимальное число одновременных игроков.","10",1,10000);
        choice("Мир",false,"difficulty","Сложность","Изменение применяется при следующем запуске.","easy",new String[]{"peaceful","easy","normal","hard"},new String[]{"Мирная","Лёгкая","Нормальная","Сложная"});
        text("Мир",false,"level-seed","Сид нового мира","Пусто — случайный. После создания мира сид здесь менять нельзя.","");
        toggle("Вход",false,"online-mode","Проверка лицензии Java","Включено — вход через подтверждённую учётную запись Minecraft.",true);
        choice("Вход",true,"offline-license-mode","Лицензия при офлайн-входе","Используется только при выключенной проверке лицензии Java.","cosmetics",new String[]{"cosmetics","disabled"},new String[]{"Проверять для скинов и плащей","Не проверять совсем"});
        toggle("Вход",true,"auth-enabled","Пароли AuthMe","Защита паролем при офлайн-входе; учётные записи хранятся в БД сервера.",true);
        toggle("Вход",true,"bedrock-online-mode","Учётная запись Xbox","Проверять подтверждение учётной записи Bedrock.",false);
        number("Производительность",false,"view-distance","Дальность обзора","В чанках. На телефоне начальное значение 6; больше — выше нагрузка.","6",3,32);
        number("Производительность",false,"simulation-distance","Дальность симуляции","Область активного игрового мира, в чанках.","4",3,32);
        number("Производительность",false,"pause-when-empty-seconds","Пауза без игроков","Через сколько секунд заморозить мир. -1 — не замораживать.","300",-1,86400);
        toggle("Производительность",true,"parallel-dimension-ticks","Параллельные блочные тики","Экспериментально: часть обработки блоков измерений; не ускоряет всё ядро или TNT.",false);
        number("Производительность",true,"dimension-tick-threads","Предел потоков блочных тиков","0 — автоматически, максимум 3. Используется для параллельных блочных тиков.","0",0,3);
        choice("Вход",true,"auth-mode","Окно авторизации","При офлайн-входе: отдельный диалог Java или команды в чате.","dialog",new String[]{"dialog","chat"},new String[]{"Диалог","Команды в чате"});
        toggle("Вход",false,"prevent-proxy-connections","Проверять адрес при входе","Передавать адрес клиента при проверке лицензионной Java-сессии. Для обычной игры оставь выключенным.",false);
        toggle("Мир",true,"autumn-forests","Осенние леса","Наши редкие осенние участки. Изменение генерации касается новых чанков.",true);
        toggle("Мир",true,"pink-cherry-groves","Розовые вишнёвые рощи","Наши оттенки травы и дополнительные лепестки в новых чанках.",true);
        number("Мир",true,"portal-delay","Ожидание в портале","В тактах: 20 = секунда, 80 = четыре секунды. Для выживания; творческий режим переходит сразу.","80",1,12000);
        number("Производительность",true,"prefetch-chunks","Чанки впрок","Сколько чанков дальше обзора готовить заранее. 0 — выключить. Больше требует больше памяти.","3",0,16);
        number("Производительность",true,"prefetch-at-once","Параллельная подготовка чанков","Сколько чанков одного игрока готовить впрок одновременно. Большие значения повышают нагрузку.","4",1,16);
        number("Сеть",false,"network-compression-threshold","Порог сжатия пакетов","В байтах. 256 — обычное значение; -1 — без сжатия. Сжатие экономит трафик за счёт CPU.","256",-1,1048576);
        toggle("Сеть",true,"text-info","Текстовая справка на порту Java","Отвечать справкой при подключении через браузер или telnet. Minecraft продолжает работать обычно.",true);
        toggle("Дополнительно",true,"tab-tps","TPS в списке игроков","Показывать TPS и время такта внизу списка Java-игроков. Плагин может заменить оформление.",true);
        toggle("Дополнительно",true,"debug","Подробный журнал","Логи пакетов и движений для поиска неполадок. Сильно увеличивает журнал; обычно не нужно.",false);
        toggle("Дополнительно",true,"send-error-reports","Анонимные отчёты об ошибках","Отправлять разработчикам отчёты для поиска сбоев. Можно отключить.",true);
        toggle("Дополнительно",true,"bedrock-show-coordinates","Координаты в Bedrock","Показывать координаты игроку Bedrock.",true);
    }
    final File root;
    final String serverOriginal, mcsaOriginal;
    final ConfigDocument server, mcsa;
    interface Writer { void write(File file,String text) throws IOException; }
    private final Writer writer;
    ServerSettings(File root) throws IOException {
        this(root,ServerSettings::write);
    }
    ServerSettings(File root,Writer writer) throws IOException {
        this.root=root;
        this.writer=writer;
        serverOriginal=read(new File(root,"config/server.properties"));
        mcsaOriginal=read(new File(root,"config/mcsa.properties"));
        server=new ConfigDocument(serverOriginal); mcsa=new ConfigDocument(mcsaOriginal);
    }
    String value(Field field) { return (field.mcsa?mcsa:server).get(field.key,field.fallback); }
    Map<String,String> values() {
        Map<String,String> result=new LinkedHashMap<>();
        for(Field field:FIELDS) result.put(field.key,value(field));
        return result;
    }
    void save(Map<String,String> values) throws IOException {
        synchronized(HostState.LOCK) {
            if(HostState.phase!=HostState.Phase.STOPPED) throw new IOException("Сначала останови сервер: настройки применяются при запуске.");
            File sp=new File(root,"config/server.properties"), mp=new File(root,"config/mcsa.properties");
            if(!read(sp).equals(serverOriginal)||!read(mp).equals(mcsaOriginal)) throw new IOException("Конфиги изменились. Открой настройки заново, чтобы не потерять изменения.");
            Map<String,String> a=new LinkedHashMap<>(),b=new LinkedHashMap<>();
            for(Field f:FIELDS) {
                String v=values.get(f.key); if(v==null) throw new IllegalArgumentException("Нет значения: "+f.title);
                f.validate(v); (f.mcsa?b:a).put(f.key,v);
            }
            if(new File(root,"world").exists()&&!values.get("level-seed").equals(server.get("level-seed","")))
                throw new IllegalArgumentException("У существующего мира нельзя сменить сид. Мир сохранён.");
            b.put("java-version","26.3"); b.put("console-colors","never");
            writer.write(sp,server.with(a));
            try { writer.write(mp,mcsa.with(b)); }
            catch(IOException error) { try { writer.write(sp,serverOriginal); } catch(IOException rollback) { error.addSuppressed(rollback); } throw error; }
        }
    }
    static String read(File file) throws IOException {
        if(!file.exists()&&!new File(file.getPath()+".bak").exists()) return "";
        try(FileInputStream in=new AtomicFile(file).openRead(); ByteArrayOutputStream out=new ByteArrayOutputStream()) {
            byte[] bytes=new byte[4096]; int n;
            while((n=in.read(bytes))!=-1) { out.write(bytes,0,n); if(out.size()>1024*1024) throw new IOException("Конфиг слишком большой"); }
            return new String(out.toByteArray(),StandardCharsets.UTF_8);
        }
    }
    static void write(File file,String text) throws IOException {
        File parent=file.getParentFile(); if(!parent.isDirectory()&&!parent.mkdirs()) throw new IOException("Не удалось создать папку настроек");
        AtomicFile atomic=new AtomicFile(file); FileOutputStream stream=atomic.startWrite();
        try { stream.write(text.getBytes(StandardCharsets.UTF_8)); stream.getFD().sync(); atomic.finishWrite(stream); }
        catch(IOException error) { atomic.failWrite(stream); throw error; }
    }
}
