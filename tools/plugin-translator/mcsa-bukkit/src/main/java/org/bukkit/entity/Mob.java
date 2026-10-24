package org.bukkit.entity;
public interface Mob extends LivingEntity {
 default LivingEntity getTarget(){String uuid=ru.mcsa.bukkit.Host.text(111,getUniqueId().toString());return uuid.isEmpty()?null:(LivingEntity)ru.mcsa.bukkit.RemoteEntity.find(uuid);}
 default void setTarget(LivingEntity target){ru.mcsa.bukkit.Host.call(112,getUniqueId().toString(),target==null?"":target.getUniqueId().toString());}
}
