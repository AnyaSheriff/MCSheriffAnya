package org.bukkit.configuration;
import java.util.*;
public interface ConfigurationSection {
 Object get(String path);void set(String path,Object value);boolean contains(String path);Set<String> getKeys(boolean deep);ConfigurationSection getConfigurationSection(String path);
 String getString(String path);String getString(String path,String fallback);int getInt(String path);int getInt(String path,int fallback);long getLong(String path);long getLong(String path,long fallback);double getDouble(String path);double getDouble(String path,double fallback);boolean getBoolean(String path);boolean getBoolean(String path,boolean fallback);List<String> getStringList(String path);List<Integer> getIntegerList(String path);
}
