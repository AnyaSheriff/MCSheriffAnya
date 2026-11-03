package net.kyori.adventure.util;
public enum TriState { NOT_SET,FALSE,TRUE;
 public static TriState byBoolean(boolean value){return value?TRUE:FALSE;}
 public static TriState byBoolean(Boolean value){return value==null?NOT_SET:byBoolean(value.booleanValue());}
 public Boolean toBoolean(){return this==NOT_SET?null:this==TRUE;}
 public boolean toBooleanOrElse(boolean other){return this==NOT_SET?other:this==TRUE;}
 public boolean toBooleanOrElseGet(java.util.function.BooleanSupplier other){return this==NOT_SET?other.getAsBoolean():this==TRUE;}
}
