package org.bukkit.configuration.file;
import java.util.*;
import org.bukkit.configuration.*;
public class FileConfiguration implements ConfigurationSection {
 protected Map<String,Object> values=new LinkedHashMap<>();
 public void loadFromString(String text){values=ru.mcsa.bukkit.Yaml.parse(text);}
 private String[] parts(String path){if(path==null||path.isEmpty())throw new IllegalArgumentException("Пустой путь конфигурации");return ru.mcsa.bukkit.Strings.split(path,'.');}
 @SuppressWarnings("unchecked") private Map<String,Object> map(Object value){return (Map<String,Object>)value;}
 private Object raw(String path){Object result=values;for(String part:parts(path)){if(!(result instanceof Map))return null;result=((Map<?,?>)result).get(part);}return result;}
 public Object get(String path){Object value=raw(path);if(value instanceof Map){FileConfiguration section=new FileConfiguration();section.values=map(value);return section;}return value;}
 public ConfigurationSection getConfigurationSection(String path){Object result=get(path);return result instanceof ConfigurationSection?(ConfigurationSection)result:null;}
 public void set(String path,Object value){String[] keys=parts(path);Map<String,Object> target=values;for(int i=0;i<keys.length-1;i++){Object child=target.get(keys[i]);if(!(child instanceof Map)){if(value==null)return;child=new LinkedHashMap<String,Object>();target.put(keys[i],child);}target=map(child);}if(value==null)target.remove(keys[keys.length-1]);else target.put(keys[keys.length-1],value instanceof FileConfiguration?((FileConfiguration)value).values:value);}
 public boolean contains(String path){return raw(path)!=null;}
 public Set<String> getKeys(boolean deep){Set<String> keys=new LinkedHashSet<>();collect(values,"",deep,keys);return keys;}
 private void collect(Map<String,Object> map,String prefix,boolean deep,Set<String> keys){for(Map.Entry<String,Object> e:map.entrySet()){String path=prefix+e.getKey();keys.add(path);if(deep&&e.getValue() instanceof Map)collect(map(e.getValue()),path+".",true,keys);}}
 public String getString(String path){return getString(path,null);}public String getString(String path,String fallback){Object v=raw(path);return v==null?fallback:String.valueOf(v);}
 public int getInt(String path){return getInt(path,0);}public int getInt(String path,int fallback){Object v=raw(path);return v instanceof Number?((Number)v).intValue():fallback;}
 public long getLong(String path){return getLong(path,0);}public long getLong(String path,long fallback){Object v=raw(path);return v instanceof Number?((Number)v).longValue():fallback;}
 public double getDouble(String path){return getDouble(path,0);}public double getDouble(String path,double fallback){Object v=raw(path);return v instanceof Number?((Number)v).doubleValue():fallback;}
 public boolean getBoolean(String path){return getBoolean(path,false);}public boolean getBoolean(String path,boolean fallback){Object v=raw(path);return v instanceof Boolean?(Boolean)v:fallback;}
 public List<String> getStringList(String path){List<String> result=new ArrayList<>();Object v=raw(path);if(v instanceof List)for(Object item:(List<?>)v)if(item instanceof String||item instanceof Number||item instanceof Boolean)result.add(String.valueOf(item));return result;}
 public List<Integer> getIntegerList(String path){List<Integer> result=new ArrayList<>();Object v=raw(path);if(v instanceof List)for(Object item:(List<?>)v)if(item instanceof Number)result.add(((Number)item).intValue());return result;}
 public String saveToString(){return ru.mcsa.bukkit.Yaml.write(values)+"\n";}
 public void load(java.io.File file)throws java.io.IOException,InvalidConfigurationException{try{if(!ru.mcsa.bukkit.Host.yes(34,file.toString().substring("__plugin_data__/".length())))throw new java.io.IOException("Файл отсутствует");loadFromString(ru.mcsa.bukkit.Host.text(8,file.toString().substring("__plugin_data__/".length())));}catch(IllegalArgumentException e){throw new InvalidConfigurationException(e);}catch(IllegalStateException e){throw new java.io.IOException(e.getMessage());}}
 public void save(java.io.File file)throws java.io.IOException{try{ru.mcsa.bukkit.Host.call(9,file.toString().substring("__plugin_data__/".length()),saveToString());}catch(IllegalStateException e){throw new java.io.IOException(e.getMessage());}}
}
