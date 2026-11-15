package ru.sheriffanya.mcsa;

import org.json.JSONArray;
import org.json.JSONObject;
import java.io.IOException;
import java.util.*;

/** Только описания обычных полей: никаких исполняемых действий из сети. */
final class SettingsSchema {
    final List<ServerSettings.Field> fields;
    final String diagnostic;
    private SettingsSchema(List<ServerSettings.Field> extra,String diagnostic) {
        List<ServerSettings.Field> all=new ArrayList<>(ServerSettings.FIELDS);all.addAll(extra);
        fields=Collections.unmodifiableList(all);this.diagnostic=diagnostic;
    }
    static SettingsSchema base() { return new SettingsSchema(Collections.emptyList(),""); }
    static SettingsSchema unavailable() { return new SettingsSchema(Collections.emptyList(),"Описание настроек выбранного ядра недоступно. Показаны основные настройки."); }
    static SettingsSchema parse(JSONObject payload) {
        if(!payload.has("settings")) return base();
        try { return decode(payload.getJSONObject("settings")); }
        catch(Exception invalid) { return new SettingsSchema(Collections.emptyList(),"Дополнительные настройки ядра отклонены: неверное описание. Показаны основные настройки."); }
    }
    private static String text(JSONObject object,String key,int max,boolean empty) throws Exception {
        Object raw=object.get(key);if(!(raw instanceof String)) throw new IOException("Ожидался текст");
        String value=(String)raw;
        if((!empty&&value.trim().isEmpty())||value.length()>max||value.indexOf('\r')>=0||value.indexOf('\n')>=0||value.indexOf('\0')>=0) throw new IOException("Неверный текст");
        return value;
    }
    private static int integer(JSONObject object,String key) throws Exception {
        Object value=object.get(key);if(!(value instanceof Integer)&&!(value instanceof Long)) throw new IOException("Ожидалось целое число");
        long number=((Number)value).longValue();if(number<Integer.MIN_VALUE||number>Integer.MAX_VALUE) throw new IOException("Число вне диапазона");return (int)number;
    }
    private static SettingsSchema decode(JSONObject data) throws Exception {
        if(data.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8).length>35000) throw new IOException("Описание слишком большое");
        if(integer(data,"format")!=1) throw new IOException("Неизвестный формат");
        JSONArray array=data.getJSONArray("fields");if(array.length()>128) throw new IOException("Слишком много полей");
        Set<String> keys=new HashSet<>();for(ServerSettings.Field f:ServerSettings.FIELDS) keys.add(f.key);
        // Эти параметры приложение устанавливает само; описание ядра не может заменить их.
        keys.add("java-version");keys.add("console-colors");
        List<ServerSettings.Field> result=new ArrayList<>();
        for(int i=0;i<array.length();i++) {
            JSONObject item=array.getJSONObject(i);
            String file=text(item,"file",6,false),key=text(item,"key",80,false);
            if((!file.equals("mcsa")&&!file.equals("server"))||!key.matches("[a-z][a-z0-9-]{0,79}")||!keys.add(key)) throw new IOException("Неверный ключ");
            String group=text(item,"group",80,false),title=text(item,"title",160,false),help=text(item,"help",2000,true),type=text(item,"type",16,false);
            if(Arrays.asList("Оформление","Резервные копии","Обновления","Обновления ядра","Обновления приложения").contains(group)) throw new IOException("Зарезервированный раздел");
            String fallback=text(item,"default",2048,true);int min=Integer.MIN_VALUE,max=0;String[] choices=null,labels=null;
            switch(type) {
                case "boolean": choices=new String[]{"false","true"};labels=new String[]{"Выключено","Включено"};break;
                case "integer":
                    min=item.has("min")?integer(item,"min"):Integer.MIN_VALUE+1;max=item.has("max")?integer(item,"max"):Integer.MAX_VALUE;
                    // MIN_VALUE используется прежним редактором как признак текстового поля.
                    if(min==Integer.MIN_VALUE||min>max) throw new IOException("Неверные границы");break;
                case "text": break;
                case "choice":
                    JSONArray options=item.getJSONArray("choices");if(options.length()<1||options.length()>64) throw new IOException("Неверный список");
                    choices=new String[options.length()];labels=new String[options.length()];Set<String> unique=new HashSet<>();
                    for(int n=0;n<options.length();n++) { JSONObject option=options.getJSONObject(n);choices[n]=text(option,"value",2048,true);labels[n]=text(option,"label",160,false);if(!unique.add(choices[n])) throw new IOException("Повтор значения"); }break;
                default: throw new IOException("Неизвестный тип");
            }
            ServerSettings.Field field=new ServerSettings.Field(group,file.equals("mcsa"),key,title,help,fallback,min,max,choices,labels);field.validate(fallback);result.add(field);
        }
        return new SettingsSchema(result,"");
    }
}
