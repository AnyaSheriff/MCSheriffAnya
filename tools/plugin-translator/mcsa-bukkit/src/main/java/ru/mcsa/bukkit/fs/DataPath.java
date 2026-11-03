package ru.mcsa.bukkit.fs;
final class DataPath implements Path {
 final String name;DataPath(String name){this.name=File.check(name);}
 public Path getParent(){int i=name.lastIndexOf('/');return name.isEmpty()?null:new DataPath(i<0?"":name.substring(0,i));}
 public Path resolveSibling(String child){int i=name.lastIndexOf('/');return new DataPath((i<0?"":name.substring(0,i+1))+child);}
 public File toFile(){return new File(File.ROOT+(name.isEmpty()?"":"/"+name));}public String toString(){return toFile().getPath();}
}
