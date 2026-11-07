package ru.sheriffanya.mcsa;

import org.junit.Test;
import static org.junit.Assert.*;
import java.util.LinkedHashMap;
import java.util.Map;

public class ConfigDocumentTest {
    private Map<String,String> change(String key,String value) { Map<String,String> map=new LinkedHashMap<>(); map.put(key,value); return map; }
    @Test public void commentsUnknownFieldsAndRussianSurvive() {
        String source="# Параметры владельца\nowner-option=свой вариант\nmotd=До\n";
        String edited=new ConfigDocument(source).with(change("motd","После"));
        assertEquals("# Параметры владельца\nowner-option=свой вариант\nmotd=После\n",edited);
    }
    @Test public void duplicateKeysCannotRestoreAnOldValue() {
        ConfigDocument doc=new ConfigDocument("port=1\n#port=2\nport=3\n");
        assertEquals("3",doc.get("port",""));
        assertEquals("port=7\n#port=2\nport=7\n",doc.with(change("port","7")));
    }
    @Test public void equalsAndBackslashesAreNotJavaPropertiesEscapes() {
        ConfigDocument doc=new ConfigDocument("motd=  Привет=мир\\n  \n");
        assertEquals("Привет=мир\\n",doc.get("motd",""));
    }
    @Test public void emptyDocumentAndMissingFinalNewlineAreSupported() {
        assertEquals("port=7\n",new ConfigDocument("").with(change("port","7")));
        assertEquals("unknown=x\nport=7\n",new ConfigDocument("unknown=x").with(change("port","7")));
    }
    @Test public void onlyHashStartsACommentAsInTheCore() {
        assertEquals("yes",new ConfigDocument(";key=yes\n!key=other\n").get(";key","no"));
    }
    @Test public void lineAndNullInjectionAreRejected() {
        for(String value:new String[]{"x\ny=1","x\ry=1","x\0y"}) {
            try { new ConfigDocument("motd=old\n").with(change("motd",value)); fail("accepted a control character"); }
            catch(IllegalArgumentException expected) { }
        }
    }
}
