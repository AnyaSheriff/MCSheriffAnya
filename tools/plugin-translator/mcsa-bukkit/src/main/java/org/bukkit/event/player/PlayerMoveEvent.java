package org.bukkit.event.player;
import org.bukkit.Location;import org.bukkit.entity.Player;
public class PlayerMoveEvent extends PlayerEvent implements org.bukkit.event.Cancellable {
 private Location from,to;private boolean cancelled;
 public PlayerMoveEvent(Player player,Location from,Location to){super(player);setFrom(from);this.to=to;}
 public Location getFrom(){return from;}public Location getTo(){return to;}
 public void setFrom(Location value){if(value==null)throw new IllegalArgumentException("Место отправления не задано");from=value;}
 public void setTo(Location value){if(value==null)throw new IllegalArgumentException("Место назначения не задано");to=value;}
 public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
 private boolean world(){return to!=null&&!from.getWorld().getName().equals(to.getWorld().getName());}
 public boolean hasChangedBlock(){return to!=null&&(world()||from.getBlockX()!=to.getBlockX()||from.getBlockY()!=to.getBlockY()||from.getBlockZ()!=to.getBlockZ());}
 public boolean hasChangedPosition(){return to!=null&&(world()||from.getX()!=to.getX()||from.getY()!=to.getY()||from.getZ()!=to.getZ());}
 public boolean hasChangedOrientation(){return to!=null&&(from.getYaw()!=to.getYaw()||from.getPitch()!=to.getPitch());}
}
