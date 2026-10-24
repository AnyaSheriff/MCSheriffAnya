package ru.mcsa.bukkit.fs;
public interface Path {Path getParent();Path resolveSibling(String name);File toFile();String toString();}
