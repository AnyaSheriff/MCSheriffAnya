package ru.sheriffanya.mcsa;

import org.junit.Test;
import org.json.*;
import java.io.File;
import java.util.Map;
import androidx.test.core.app.ApplicationProvider;
import static org.junit.Assert.*;

public class SettingsSchemaTest {
    private JSONObject field() throws Exception { return new JSONObject().put("file","mcsa").put("key","future-setting").put("group","Новые").put("title","Новое поле").put("help","").put("type","integer").put("default","3").put("min",0).put("max",5); }
    private JSONObject payload(JSONObject field) throws Exception { return new JSONObject().put("settings",new JSONObject().put("format",1).put("fields",new JSONArray().put(field))); }
    private void rejected(JSONObject payload) { SettingsSchema schema=SettingsSchema.parse(payload);assertEquals(28,schema.fields.size());assertFalse(schema.diagnostic.isEmpty()); }
    @Test public void oldPayloadAndExplicitBaseContainOnlyBaseFields() throws Exception {
        assertEquals(28,SettingsSchema.parse(new JSONObject()).fields.size());
        assertEquals(29,SettingsSchema.parse(payload(field())).fields.size());
        assertEquals(28,SettingsSchema.base().fields.size());
        assertTrue(SettingsSchema.parse(new JSONObject()).diagnostic.isEmpty());
    }
    @Test public void invalidExtensionsAreRejectedEntirely() throws Exception {
        rejected(payload(field().put("default","6")));
        rejected(payload(field().put("min",6)));
        rejected(payload(field().put("min",0.5)));
        rejected(payload(field().put("key","debug")));
        rejected(payload(field().put("key","java-version")));
        rejected(payload(field().put("key","bad\nkey")));
        rejected(payload(field().put("default","3\nnew=1")));
        rejected(payload(field().put("file","../server")));
        rejected(payload(field().put("type","script")));
        rejected(new JSONObject().put("settings","invalid"));
        JSONObject duplicate=payload(field());duplicate.getJSONObject("settings").getJSONArray("fields").put(field());rejected(duplicate);
        JSONObject tooMany=payload(field());JSONArray list=tooMany.getJSONObject("settings").getJSONArray("fields");for(int i=0;i<128;i++) list.put(field().put("key","setting-"+i));rejected(tooMany);
    }
    @Test public void choicesAndOptionalBounds() throws Exception {
        JSONObject f=field().put("type","choice").put("default","a").put("choices",new JSONArray().put(new JSONObject().put("value","a").put("label","А")));
        assertEquals(29,SettingsSchema.parse(payload(f)).fields.size());
        f.getJSONArray("choices").put(new JSONObject().put("value","a").put("label","Ещё"));rejected(payload(f));
        JSONObject number=field();number.remove("min");number.remove("max");assertEquals(29,SettingsSchema.parse(payload(number)).fields.size());
    }
    @Test public void newFieldSavePreservesUnknownSettingsAndComments() throws Exception {
        android.content.Context context=ApplicationProvider.getApplicationContext();
        File root=new File(context.getCacheDir(),"settings-schema-"+System.nanoTime());assertTrue(root.mkdirs());
        try {
            ServerSettings.write(new File(root,"config/mcsa.properties"),"# Мой комментарий\nowner-unknown=оставить\nfuture-setting=2\n");
            ServerSettings settings=new ServerSettings(root,ServerSettings::write,SettingsSchema.parse(payload(field())));
            Map<String,String> values=settings.values();values.put("future-setting","4");settings.save(values);
            String saved=ServerSettings.read(new File(root,"config/mcsa.properties"));
            assertTrue(saved.contains("# Мой комментарий\nowner-unknown=оставить\nfuture-setting=4\n"));
            assertTrue(settings.groups().contains("Новые"));
        } finally { new File(root,"config/server.properties").delete();new File(root,"config/mcsa.properties").delete();new File(root,"config").delete();root.delete(); }
    }
}
