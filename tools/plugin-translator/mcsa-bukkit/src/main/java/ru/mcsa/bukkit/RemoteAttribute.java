package ru.mcsa.bukkit;
import org.bukkit.attribute.*;
public final class RemoteAttribute implements AttributeInstance {
 private final String uuid,name;public RemoteAttribute(String uuid,Attribute attribute){this.uuid=uuid;name=attribute.getKey().getKey();}
 private String[] snapshot(){return Host.call(42,uuid,name);}public double getBaseValue(){return Double.parseDouble(snapshot()[0]);}public double getValue(){return Double.parseDouble(snapshot()[1]);}public void setBaseValue(double value){Host.call(43,uuid,name,""+value);}
 public java.util.Collection<AttributeModifier> getModifiers(){String[] data=snapshot();java.util.List<AttributeModifier> out=new java.util.ArrayList<>();for(int i=2;i<data.length;i+=3)out.add(new AttributeModifier(org.bukkit.NamespacedKey.fromString(data[i]),Double.parseDouble(data[i+1]),AttributeModifier.Operation.values()[Integer.parseInt(data[i+2])]));return java.util.Collections.unmodifiableList(out);}
 public AttributeModifier getModifier(net.kyori.adventure.key.Key key){for(AttributeModifier modifier:getModifiers())if(modifier.getKey().asString().equals(key.asString()))return modifier;return null;}
 public void addModifier(AttributeModifier modifier){Host.call(44,uuid,name,modifier.getKey().toString(),""+modifier.getAmount(),""+modifier.getOperation().ordinal());}public void addTransientModifier(AttributeModifier modifier){addModifier(modifier);}
 public void removeModifier(AttributeModifier modifier){removeModifier(modifier.getKey());}public void removeModifier(net.kyori.adventure.key.Key key){Host.call(45,uuid,name,key.asString());}
}
