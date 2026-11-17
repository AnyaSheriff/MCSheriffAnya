package ru.mcsa.modapi;

/** Свой исходный API MCSA. Не интерфейс Fabric/Forge и не загрузчик JAR. */
public interface Mod {
    void initialize(Context context);
}
