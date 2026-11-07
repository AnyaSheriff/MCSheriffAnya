package ru.sheriffanya.mcsa;

import java.io.*;
import java.nio.charset.StandardCharsets;
import java.text.SimpleDateFormat;
import java.util.*;
import java.util.zip.*;

/** Наш ZIP-формат; мир заменяется только после полной проверки временной копии. */
final class BackupStore {
    private static final byte[] FORMAT="MCSheriffAnya backup\nformat=1\ngame=26.3\n".getBytes(StandardCharsets.UTF_8);
    static final class Limits {
        final long archive,total,file; final int entries;
        Limits(long archive,long total,long file,int entries) { this.archive=archive;this.total=total;this.file=file;this.entries=entries; }
    }
    static final Limits DEFAULT_LIMITS=new Limits(2L*1024*1024*1024,4L*1024*1024*1024,1024L*1024*1024,50000);
    private final File home,root,backups,stage,previous,transaction;
    private final Limits limits;
    BackupStore(File home) throws IOException { this(home,DEFAULT_LIMITS); }
    BackupStore(File home,Limits limits) throws IOException {
        this.home=home.getCanonicalFile();this.limits=limits;root=new File(this.home,"server");backups=new File(this.home,"backups");
        stage=new File(this.home,"server.importing");previous=new File(this.home,"server.previous");transaction=new File(this.home,"import.transaction");
    }
    private void stopped() throws IOException {
        synchronized(HostState.LOCK) { if(HostState.phase!=HostState.Phase.STOPPED) throw new IOException("Сначала сохрани и останови сервер."); }
    }
    /** Вызывается перед запуском ядра и операциями копирования, пока сервер остановлен. */
    void recover() throws IOException { stopped();recoverRoot();cleanupRecovery(); }
    void recoverForLaunch() throws IOException { stopped();recoverRoot(); }
    private void recoverRoot() throws IOException {
        if(transaction.exists()&&previous.exists()&&root.exists()) {
            // Завершение импорта ещё не отмечено: сохраняем новую папку отдельно и возвращаем прежнюю.
            File abandoned=new File(home,"server.abandoned-"+UUID.randomUUID());
            if(!root.renameTo(abandoned)) throw new IOException("Не удалось отложить незавершённое восстановление.");
        }
        if(previous.exists()&&!root.exists()&&!previous.renameTo(root)) throw new IOException("Не удалось вернуть предыдущую папку сервера. Данные сохранены.");
        if(transaction.exists()&&!transaction.delete()) throw new IOException("Не удалось завершить восстановление состояния.");
    }
    /** Объёмную уборку выполняет worker, а не главный поток Activity/Service. */
    void cleanupRecovery() throws IOException {
        if(previous.exists()&&root.exists()) remove(previous);
        if(stage.exists()) remove(stage);
        File[] abandoned=home.listFiles((dir,name)->name.matches("server\\.abandoned-[a-f0-9-]{36}"));
        if(abandoned!=null) for(File file:abandoned) remove(file);
        File[] leftovers=home.listFiles((dir,name)->name.matches("import-[a-f0-9-]{36}\\.part"));
        if(leftovers!=null) for(File file:leftovers) remove(file);
        File[] parts=backups.listFiles((dir,name)->name.matches("mcsa-[A-Za-z0-9_.-]+\\.zip\\.part"));
        if(parts!=null) for(File file:parts) remove(file);
    }
    List<File> list() {
        File[] files=backups.listFiles((dir,name)->name.matches("mcsa-[A-Za-z0-9_.-]+\\.zip"));
        List<File> result=new ArrayList<>(); if(files!=null) for(File file:files) if(file.isFile()) result.add(file);
        result.sort((a,b)->b.getName().compareTo(a.getName())); return result;
    }
    File file(String name) throws IOException {
        if(name==null||!name.matches("mcsa-[A-Za-z0-9_.-]+\\.zip")) throw new IOException("Неверное имя копии.");
        File file=new File(backups,name);
        if(!file.isFile()||!file.getCanonicalFile().equals(file.getAbsoluteFile())) throw new IOException("Копия не найдена.");
        return file;
    }
    private void validateServer(File candidate) throws IOException {
        for(String name:new String[]{"config/server.properties","config/mcsa.properties"})
            if(!new File(candidate,name).isFile()||new File(candidate,name).length()==0) throw new IOException("В копии нет полного сохранённого мира и конфигов. Сохрани настройки или запусти и штатно останови сервер.");
        if(new File(candidate,"world").exists()&&new File(candidate,"world/level.dat").length()==0) throw new IOException("Мир в копии не сохранён полностью: отсутствует level.dat.");
        String version=new ConfigDocument(ServerSettings.read(new File(candidate,"config/mcsa.properties"))).get("java-version","26.3");
        if(!version.equals("26.3")) throw new IOException("Копия использует другую основную версию игры: "+version);
    }
    File create() throws IOException {
        stopped(); validateServer(root);
        if(!backups.isDirectory()&&!backups.mkdirs()) throw new IOException("Не удалось создать папку копий.");
        if(!backups.getCanonicalFile().equals(backups.getAbsoluteFile())) throw new IOException("Папка копий не должна быть ссылкой.");
        String name="mcsa-"+new SimpleDateFormat("yyyyMMdd-HHmmss",Locale.ROOT).format(new Date())+"-"+UUID.randomUUID().toString().substring(0,8)+".zip";
        File destination=new File(backups,name),part=new File(backups,name+".part");
        if(destination.exists()||!part.createNewFile()) throw new IOException("Не удалось выбрать свободное имя копии.");
        boolean done=false;
        try {
            try(FileOutputStream stream=new FileOutputStream(part)) {
                // Не закрываем основной stream до sync: ZIP уже полностью записан finish().
                ZipOutputStream zip=new ZipOutputStream(new LimitedOutputStream(stream,limits.archive));
                try {
                    zip.putNextEntry(new ZipEntry("mcsa-backup.txt"));zip.write(FORMAT);zip.closeEntry();
                    long[] count={0,0}; pack(zip,root.getCanonicalFile(),root.getCanonicalFile(),count);zip.finish();stream.getFD().sync();
                } finally { zip.close(); }
            }
            if(!part.renameTo(destination)) throw new IOException("Не удалось завершить запись копии.");
            done=true;return destination;
        } finally { if(!done&&part.exists()&&!part.delete()) part.deleteOnExit(); }
    }
    private void pack(ZipOutputStream zip,File base,File directory,long[] count) throws IOException {
        File[] files=directory.listFiles();if(files==null) throw new IOException("Не удалось прочитать папку сервера.");
        Arrays.sort(files,Comparator.comparing(File::getName));
        for(File item:files) {
            if(!item.getCanonicalFile().equals(item.getAbsoluteFile())) throw new IOException("Символические ссылки в копиях не поддерживаются.");
            String relative=item.getAbsolutePath().substring(base.getAbsolutePath().length()+1);
            validName("server/"+relative+(item.isDirectory()?"/":""));
            if(++count[1]>limits.entries) throw new IOException("Слишком много файлов: максимум "+limits.entries);
            if(item.isDirectory()) {
                zip.putNextEntry(new ZipEntry("server/"+relative+"/"));zip.closeEntry();pack(zip,base,item,count);
            } else {
                if(!item.isFile()) throw new IOException("В папке есть неподдерживаемый тип файла.");
                zip.putNextEntry(new ZipEntry("server/"+relative));
                try(InputStream input=new FileInputStream(item)) { transfer(input,zip,limits.file,count); }
                zip.closeEntry();
            }
        }
    }
    /** Восстановление из внутренней/выбранной через SAF копии; возвращает имя автокопии. */
    String restore(InputStream input) throws IOException {
        stopped();recover();
        if(!home.isDirectory()&&!home.mkdirs()) throw new IOException("Не удалось создать папку приложения.");
        File archive=new File(home,"import-"+UUID.randomUUID()+".part");
        try {
            try(FileOutputStream out=new FileOutputStream(archive)) { transfer(input,out,limits.archive,null);out.getFD().sync(); }
            if(!stage.mkdir()) throw new IOException("Не удалось подготовить папку восстановления.");
            unpack(archive);validateServer(stage);
            String automatic=null;
            if(root.exists()) {
                automatic=create().getName();
                try(FileOutputStream mark=new FileOutputStream(transaction)) { mark.write(1);mark.getFD().sync(); }
                if(!root.renameTo(previous)) throw new IOException("Не удалось отложить текущий мир. Он не заменён.");
            }
            if(!stage.renameTo(root)) {
                if(previous.exists()&&!previous.renameTo(root)) throw new IOException("Не удалось вернуть папку сервера; предыдущие данные сохранены в server.previous и ZIP.");
                throw new IOException("Не удалось заменить папку сервера. Прежние данные сохранены.");
            }
            // Без отметки завершения после сбоя возвращаем старую папку. Готовый ZIP прежнего мира остаётся всегда.
            if(transaction.exists()&&!transaction.delete()) throw new IOException("Данные восстановлены, но операция ещё не завершена. При следующем запуске вернётся прежний мир.");
            if(previous.exists()) remove(previous);
            return automatic;
        } finally {
            if(archive.exists()&&!archive.delete()) archive.deleteOnExit();
            if(stage.exists()) remove(stage);
        }
    }
    private void unpack(File archive) throws IOException {
        // Наш writer не создаёт ZIP64/многотомные ZIP. Проверяем размер каталога до выделений ZipFile.
        try(RandomAccessFile file=new RandomAccessFile(archive,"r")) {
            if(file.length()<22) throw new IOException("Неполный ZIP.");byte[] end=new byte[22];file.seek(file.length()-22);file.readFully(end);
            int entries=(end[10]&255)|((end[11]&255)<<8),diskEntries=(end[8]&255)|((end[9]&255)<<8);
            long size=0,offset=0;for(int i=0;i<4;i++) { size|=(long)(end[12+i]&255)<<(8*i);offset|=(long)(end[16+i]&255)<<(8*i); }
            if(end[0]!=0x50||end[1]!=0x4b||end[2]!=5||end[3]!=6||end[4]!=0||end[5]!=0||end[6]!=0||end[7]!=0||end[20]!=0||end[21]!=0||entries!=diskEntries||entries>limits.entries+1||size>64L*1024*1024||offset+size+22!=file.length())
                throw new IOException("Неполный или неподдерживаемый каталог ZIP. Выбери копию приложения.");
        }
        Map<String,ZipEntry> central=new HashMap<>();
        try(ZipFile file=new ZipFile(archive)) {
            Enumeration<? extends ZipEntry> entries=file.entries();
            while(entries.hasMoreElements()) {
                ZipEntry entry=entries.nextElement();String name=entry.getName();
                if(!name.equals("mcsa-backup.txt")) validName(name);
                if(central.put(name,entry)!=null||central.size()>limits.entries+1) throw new IOException("Повторяющиеся пути или слишком много файлов в ZIP.");
            }
        }
        Set<String> names=new HashSet<>();long[] count={0,0};boolean marker=false;
        try(ZipInputStream zip=new ZipInputStream(new FileInputStream(archive))) {
            ZipEntry entry;
            while((entry=zip.getNextEntry())!=null) {
                String name=entry.getName();
                if(!central.containsKey(name)) throw new IOException("Повреждён каталог ZIP.");
                if(!names.add(name)||++count[1]>limits.entries+1) throw new IOException("Повторяющиеся пути или слишком много файлов в ZIP.");
                if(name.equals("mcsa-backup.txt")) {
                    ByteArrayOutputStream bytes=new ByteArrayOutputStream();transfer(zip,bytes,FORMAT.length,null);
                    if(!Arrays.equals(FORMAT,bytes.toByteArray())) throw new IOException("Неизвестный формат копии или версия игры.");marker=true;
                } else {
                    String relative=validName(name);
                    File target=new File(stage,relative);
                    if(!target.getCanonicalPath().startsWith(stage.getCanonicalPath()+File.separator)) throw new IOException("Путь выходит за папку копии.");
                    if(entry.isDirectory()) { if(!target.isDirectory()&&!target.mkdirs()) throw new IOException("Не удалось создать папку из копии."); }
                    else {
                        File parent=target.getParentFile();if(!parent.isDirectory()&&!parent.mkdirs()) throw new IOException("Не удалось создать папку из копии.");
                        if(target.exists()) throw new IOException("Конфликт путей в копии.");
                        if(new android.os.StatFs(home.getPath()).getAvailableBytes()<64L*1024*1024) throw new IOException("Недостаточно свободной памяти для восстановления.");
                        try(FileOutputStream out=new FileOutputStream(target)) { transfer(zip,out,limits.file,count);out.getFD().sync(); }
                    }
                }
                zip.closeEntry();
                ZipEntry expected=central.get(name);
                if(entry.getSize()!=expected.getSize()||entry.getCrc()!=expected.getCrc()) throw new IOException("Повреждённые данные ZIP.");
            }
        }
        if(!names.equals(central.keySet())) throw new IOException("Неполный ZIP.");
        if(!marker) throw new IOException("Это не копия MCSheriffAnya. Выбери ZIP, созданный приложением.");
    }
    private String validName(String name) throws IOException {
        if(name.length()>1024||!name.startsWith("server/")||name.indexOf('\\')>=0) throw new IOException("Недопустимый путь в ZIP.");
        String relative=name.substring(7);if(relative.endsWith("/")) relative=relative.substring(0,relative.length()-1);
        for(String segment:relative.split("/",-1)) {
            if(segment.isEmpty()||segment.equals(".")||segment.equals("..")) throw new IOException("Недопустимый путь в ZIP.");
            for(int i=0;i<segment.length();i++) if(Character.isISOControl(segment.charAt(i))) throw new IOException("Недопустимый путь в ZIP.");
        }
        return relative;
    }
    private void transfer(InputStream input,OutputStream out,long maximum,long[] count) throws IOException {
        byte[] buffer=new byte[32768];long size=0;int n;
        while((n=input.read(buffer))!=-1) {
            size+=n;if(size>maximum) throw new IOException("Превышен предел размера файла или архива.");
            if(count!=null) { count[0]+=n;if(count[0]>limits.total) throw new IOException("Слишком большая копия: максимум 4 ГиБ несжатых данных."); }
            out.write(buffer,0,n);
        }
    }
    private void remove(File file) throws IOException {
        if(!file.getCanonicalFile().equals(file.getAbsoluteFile())) throw new IOException("Нельзя удалять символическую ссылку из папки восстановления.");
        if(file.isDirectory()) { File[] files=file.listFiles();if(files==null) throw new IOException("Не удалось прочитать временную папку.");for(File child:files) remove(child); }
        if(!file.delete()) throw new IOException("Не удалось убрать временные данные: "+file.getName());
    }
    private static final class LimitedOutputStream extends FilterOutputStream {
        private final long maximum;private long written;
        LimitedOutputStream(OutputStream out,long maximum) { super(out);this.maximum=maximum; }
        @Override public void write(int value) throws IOException { check(1);out.write(value); }
        @Override public void write(byte[] bytes,int offset,int length) throws IOException { check(length);out.write(bytes,offset,length); }
        private void check(int n) throws IOException { written+=n;if(written>maximum) throw new IOException("Архив больше 2 ГиБ."); }
    }
}
