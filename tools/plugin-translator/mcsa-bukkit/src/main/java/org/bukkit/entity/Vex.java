package org.bukkit.entity;
public interface Vex extends Mob,Enemy {
 default void setLimitedLifetime(boolean value){ru.mcsa.bukkit.Host.call(110,getUniqueId().toString(),"1",value?"1":"0");}
 default void setLimitedLifetimeTicks(int ticks){ru.mcsa.bukkit.Host.call(110,getUniqueId().toString(),"2",""+ticks);}
}
