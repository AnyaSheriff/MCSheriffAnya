package org.bukkit.projectiles;
public interface ProjectileSource {
 default <T extends org.bukkit.entity.Projectile> T launchProjectile(Class<? extends T> type,org.bukkit.util.Vector velocity){
  if(!(this instanceof org.bukkit.entity.Entity))throw new UnsupportedOperationException("Запуск снарядов от блоков пока не поддерживается");
  String kind=type==org.bukkit.entity.Fireball.class?"fireball":type==org.bukkit.entity.SmallFireball.class?"small_fireball":type==org.bukkit.entity.WindCharge.class?"wind_charge":type==org.bukkit.entity.ThrownPotion.class?"splash_potion":null;
  if(kind==null)throw new UnsupportedOperationException("Этот класс снаряда пока не поддерживается");
  String uuid=ru.mcsa.bukkit.Host.text(102,((org.bukkit.entity.Entity)this).getUniqueId().toString(),kind,""+velocity.getX(),""+velocity.getY(),""+velocity.getZ());return type.cast(ru.mcsa.bukkit.RemoteEntity.find(uuid));
 }
}
