package org.bukkit.event.entity;
public class EntityEvent extends org.bukkit.event.Event {
 protected final org.bukkit.entity.Entity entity;public EntityEvent(org.bukkit.entity.Entity entity){this.entity=entity;}public org.bukkit.entity.Entity getEntity(){return entity;}
}
