package ru.sheriffanya.mcsa;

import androidx.test.platform.app.InstrumentationRegistry;
import org.junit.Test;
import static org.junit.Assert.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.zip.*;

public final class BackupStoreTest {
    private File home() throws IOException {
        File home=new File(InstrumentationRegistry.getInstrumentation().getTargetContext().getCacheDir(),"backup-fixture-"+UUID.randomUUID());
        assertTrue(home.mkdirs());
        ServerSettings.write(new File(home,"server/config/server.properties"),"# owner\nmax-players=7\n");
        ServerSettings.write(new File(home,"server/config/mcsa.properties"),"java-version=26.3\n");
        ServerSettings.write(new File(home,"server/world/level.dat"),"our synthetic test fixture\n");
        ServerSettings.write(new File(home,"server/world/marker"),"before\n");return home;
    }
    private String zipText(File zip,String name) throws IOException {
        try(ZipFile file=new ZipFile(zip);InputStream in=file.getInputStream(file.getEntry(name));ByteArrayOutputStream out=new ByteArrayOutputStream()) { byte[] bytes=new byte[4096];int n;while((n=in.read(bytes))!=-1) out.write(bytes,0,n);return new String(out.toByteArray(),StandardCharsets.UTF_8); }
    }
    @Test public void backupRoundTripAutoCopyAndInterruptedRenameRecovery() throws Exception {
        File home=home();BackupStore store=new BackupStore(home);File original=store.create();
        ServerSettings.write(new File(home,"server/world/marker"),"changed\n");
        String automatic;try(InputStream in=new FileInputStream(original)) { automatic=store.restore(in); }
        assertEquals("before\n",ServerSettings.read(new File(home,"server/world/marker")));
        assertEquals("changed\n",zipText(store.file(automatic),"server/world/marker"));assertEquals(2,store.list().size());
        assertFalse(new File(home,"server.importing").exists());assertFalse(new File(home,"server.previous").exists());
        assertTrue(new File(home,"server").renameTo(new File(home,"server.previous")));
        ServerSettings.write(new File(home,"server.importing/partial"),"incomplete\n");
        // Сбой после перемещения прежнего мира и появления неполной новой папки.
        ServerSettings.write(new File(home,"import.transaction"),"pending");
        ServerSettings.write(new File(home,"server/config/server.properties"),"new partial config");
        try { new ServerSettings(new File(home,"server")).save(new ServerSettings(new File(home,"server")).values());fail("wrote into pending restore"); } catch(IOException expected) { }
        store.recoverForLaunch();assertEquals("before\n",ServerSettings.read(new File(home,"server/world/marker")));
        store.cleanupRecovery();assertFalse(new File(home,"server.importing").exists());
        File link=new File(home,"server/link");android.system.Os.symlink(new File(home,"server/world/marker").getPath(),link.getPath());
        try { store.create();fail("followed symbolic link"); } catch(IOException expected) { } finally { assertTrue(link.delete()); }
        synchronized(HostState.LOCK) { HostState.phase=HostState.Phase.RUNNING; }
        try { store.create();fail("backed up running world"); } catch(IOException expected) { }
        finally { synchronized(HostState.LOCK) { HostState.phase=HostState.Phase.STOPPED; } }
    }
    private byte[] archive(String path,String value,String marker) throws IOException {
        ByteArrayOutputStream bytes=new ByteArrayOutputStream();
        try(ZipOutputStream zip=new ZipOutputStream(bytes)) {
            zip.putNextEntry(new ZipEntry("mcsa-backup.txt"));zip.write(marker.getBytes(StandardCharsets.UTF_8));zip.closeEntry();
            zip.putNextEntry(new ZipEntry(path));zip.write(value.getBytes(StandardCharsets.UTF_8));zip.closeEntry();
        }
        return bytes.toByteArray();
    }
    @Test public void invalidArchivesNeverReplaceTheExistingServer() throws Exception {
        File home=home();BackupStore store=new BackupStore(home);
        String marker="MCSheriffAnya backup\nformat=1\ngame=26.3\n";
        List<byte[]> invalid=new ArrayList<>();
        for(String path:new String[]{"server/../../outside","/server/escape","server/a/../bad","server/a\\bad","server//bad"}) invalid.add(archive(path,"bad",marker));
        invalid.add(archive("server/file","bad",marker.replace("26.3","26.1.2")));
        invalid.add(archive("server/file","missing configs",marker));
        byte[] valid;try(InputStream in=new FileInputStream(store.create());ByteArrayOutputStream out=new ByteArrayOutputStream()) { byte[] buffer=new byte[4096];int n;while((n=in.read(buffer))!=-1) out.write(buffer,0,n);valid=out.toByteArray(); }
        invalid.add(Arrays.copyOf(valid,valid.length-20));
        ByteArrayOutputStream duplicate=new ByteArrayOutputStream();
        try(ZipOutputStream zip=new ZipOutputStream(duplicate)) {
            for(String path:new String[]{"mcsa-backup.txt","server/a","server/b"}) { zip.putNextEntry(new ZipEntry(path));zip.write(path.equals("mcsa-backup.txt")?marker.getBytes(StandardCharsets.UTF_8):new byte[]{1});zip.closeEntry(); }
        }
        byte[] repeated=duplicate.toByteArray(),pattern="server/b".getBytes(StandardCharsets.UTF_8);
        for(int i=0;i<=repeated.length-pattern.length;i++) {
            boolean same=true;for(int j=0;j<pattern.length;j++) if(repeated[i+j]!=pattern[j]) same=false;
            if(same) repeated[i+pattern.length-1]='a';
        }
        invalid.add(repeated);
        byte[] corrupted=valid.clone();for(int i=40;i<Math.min(corrupted.length-40,80);i++) corrupted[i]^=0x55;invalid.add(corrupted);
        for(byte[] bytes:invalid) {
            try { store.restore(new ByteArrayInputStream(bytes));fail("accepted an invalid archive"); } catch(IOException expected) { }
            assertEquals("before\n",ServerSettings.read(new File(home,"server/world/marker")));assertFalse(new File(home,"outside").exists());assertFalse(new File(home.getParentFile(),"outside").exists());assertFalse(new File(home,"server.importing").exists());assertEquals(1,store.list().size());
        }
        BackupStore bounded=new BackupStore(home,new BackupStore.Limits(4096,100,64,100));
        try { bounded.restore(new ByteArrayInputStream(archive("server/too-large",new String(new char[200]).replace('\0','a'),marker)));fail("accepted an inflated file beyond limit"); } catch(IOException expected) { }
        assertEquals("before\n",ServerSettings.read(new File(home,"server/world/marker")));
        try { store.file("../escape.zip");fail(); } catch(IOException expected) { }
    }
}
