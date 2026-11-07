package ru.sheriffanya.mcsa;

import java.util.LinkedHashMap;
import java.util.Map;

/** Формат собственного Rust parse_properties: UTF-8, комментарии и неизвестные ключи сохраняются. */
public final class ConfigDocument {
    private final String original;
    private final Map<String, String> values = new LinkedHashMap<>();
    public ConfigDocument(String text) {
        original = text;
        for (String line : text.split("\n", -1)) {
            String key = key(line);
            if (key != null) values.put(key, line.substring(line.indexOf('=') + 1).trim());
        }
    }
    private static String key(String line) {
        String text = line.trim();
        int at = line.indexOf('=');
        if (text.startsWith("#") || at < 0) return null;
        return line.substring(0, at).trim();
    }
    public String get(String name, String fallback) { return values.containsKey(name) ? values.get(name) : fallback; }
    public String with(Map<String, String> updates) {
        for (Map.Entry<String, String> e : updates.entrySet()) {
            if (e.getValue().contains("\n") || e.getValue().contains("\r") || e.getValue().indexOf('\0') >= 0)
                throw new IllegalArgumentException("В значении не должно быть переноса строки");
        }
        StringBuilder result = new StringBuilder();
        Map<String, String> pending = new LinkedHashMap<>(updates);
        String[] lines = original.split("\n", -1);
        for (int i = 0; i < lines.length; i++) {
            if (i == lines.length - 1 && lines[i].isEmpty()) break;
            String name = key(lines[i]);
            // Меняем все дубликаты: Rust принимает последнее значение.
            if (name != null && updates.containsKey(name)) {
                result.append(name).append('=').append(updates.get(name)); pending.remove(name);
            } else result.append(lines[i]);
            result.append('\n');
        }
        for (Map.Entry<String, String> e : pending.entrySet()) result.append(e.getKey()).append('=').append(e.getValue()).append('\n');
        return result.toString();
    }
}
