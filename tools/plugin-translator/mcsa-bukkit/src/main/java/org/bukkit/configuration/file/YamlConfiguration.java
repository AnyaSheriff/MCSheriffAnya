package org.bukkit.configuration.file;
public class YamlConfiguration extends FileConfiguration {
 public static YamlConfiguration loadConfiguration(java.io.File file){YamlConfiguration config=new YamlConfiguration();try{config.load(file);}catch(java.io.IOException|org.bukkit.configuration.InvalidConfigurationException e){ru.mcsa.bukkit.Host.call(23,"Конфигурация не прочитана: "+e.getMessage());}return config;}
 public static YamlConfiguration loadConfiguration(java.io.Reader reader){YamlConfiguration config=new YamlConfiguration();StringBuilder text=new StringBuilder();char[] buf=new char[4096];try{int n;while((n=reader.read(buf))>=0){text.append(buf,0,n);if(text.length()>4*1024*1024)throw new IllegalArgumentException("Слишком большая конфигурация");}config.loadFromString(text.toString());}catch(java.io.IOException e){throw new IllegalArgumentException(e);}return config;}
}
