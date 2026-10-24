package org.bukkit.util;
public class RayTraceResult {
 private final Vector position;private final org.bukkit.block.Block block;private final org.bukkit.block.BlockFace face;private final org.bukkit.entity.Entity entity;
 public RayTraceResult(Vector position){this(position,(org.bukkit.entity.Entity)null,null);}
 public RayTraceResult(Vector position,org.bukkit.block.BlockFace face){this(position,(org.bukkit.entity.Entity)null,face);}
 public RayTraceResult(Vector position,org.bukkit.block.Block block,org.bukkit.block.BlockFace face){this.position=position.clone();this.block=block;this.face=face;this.entity=null;}
 public RayTraceResult(Vector position,org.bukkit.entity.Entity entity){this(position,entity,null);}
 public RayTraceResult(Vector position,org.bukkit.entity.Entity entity,org.bukkit.block.BlockFace face){this.position=position.clone();this.block=null;this.face=face;this.entity=entity;}
 public Vector getHitPosition(){return position.clone();}public org.bukkit.block.Block getHitBlock(){return block;}public org.bukkit.block.BlockFace getHitBlockFace(){return face;}public org.bukkit.entity.Entity getHitEntity(){return entity;}
}
