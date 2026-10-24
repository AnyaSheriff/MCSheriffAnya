package org.bukkit.event.player;
import io.papermc.paper.block.bed.*;import org.bukkit.block.Block;
public class PlayerBedEnterEvent extends PlayerEvent implements org.bukkit.event.Cancellable {
 public enum BedEnterResult {OK,NOT_POSSIBLE_HERE,NOT_POSSIBLE_NOW,TOO_FAR_AWAY,NOT_SAFE,OTHER_PROBLEM}
 private final Block bed;private final BedEnterResult result;private final BedEnterAction action;private org.bukkit.event.Event.Result use=org.bukkit.event.Event.Result.DEFAULT;
 public PlayerBedEnterEvent(org.bukkit.entity.Player player,Block bed,BedEnterResult result,BedEnterAction action){super(player);this.bed=bed;this.result=result;this.action=action;}
 public Block getBed(){return bed;}public BedEnterAction enterAction(){return action;}public BedEnterResult getBedEnterResult(){return result;}
 public org.bukkit.event.Event.Result useBed(){return use;}public void setUseBed(org.bukkit.event.Event.Result value){if(value==org.bukkit.event.Event.Result.ALLOW)throw new UnsupportedOperationException("Принудительный сон в обход правил мира не поддерживается; используйте DEFAULT или DENY");use=java.util.Objects.requireNonNull(value);}
 public boolean isCancelled(){return use==org.bukkit.event.Event.Result.DENY||use==org.bukkit.event.Event.Result.DEFAULT&&(action.problem()!=null||!action.canSleep().success());}public void setCancelled(boolean value){use=value?org.bukkit.event.Event.Result.DENY:org.bukkit.event.Event.Result.DEFAULT;}
}
