package ru.mcsa.bukkit;
import org.bukkit.entity.Player;import org.bukkit.block.Block;import io.papermc.paper.block.bed.*;import org.bukkit.event.player.PlayerBedEnterEvent;
public final class BedActions {
 public static boolean cancelled(Player player,Block clicked){
  String[] values=Host.call(63,player.getUniqueId().toString(),""+((RemoteWorld)clicked.getWorld()).dimension,""+clicked.getX(),""+clicked.getY(),""+clicked.getZ());if(values.length==0)return false;
  int problem=Integer.parseInt(values[4]);boolean spawn=values[5].equals("1"),sleep=values[6].equals("1");
  BedEnterProblem failure=problem==0?null:problem==1?BedEnterProblem.TOO_FAR_AWAY:problem==2?BedEnterProblem.OBSTRUCTED:problem==3?BedEnterProblem.NOT_SAFE:problem==4?BedEnterProblem.EXPLOSION:BedEnterProblem.OTHER;
  BedEnterAction action=new BedEnterAction(){public BedRuleResult canSetSpawn(){return spawn?BedRuleResult.ALLOWED:BedRuleResult.NEVER;}public BedRuleResult canSleep(){return sleep?BedRuleResult.ALLOWED:failure==null?BedRuleResult.TOO_MUCH_LIGHT:BedRuleResult.NEVER;}public BedEnterProblem problem(){return failure;}public net.kyori.adventure.text.Component errorMessage(){return failure==null?(sleep?null:net.kyori.adventure.text.Component.text("Спать можно только ночью")):failure.errorMessage();}};
  Block bed=new RemoteBlock(new RemoteWorld(Integer.parseInt(values[0])),Integer.parseInt(values[1]),Integer.parseInt(values[2]),Integer.parseInt(values[3]),null);
  PlayerBedEnterEvent.BedEnterResult result=problem==0?(sleep?PlayerBedEnterEvent.BedEnterResult.OK:PlayerBedEnterEvent.BedEnterResult.NOT_POSSIBLE_NOW):problem==1?PlayerBedEnterEvent.BedEnterResult.TOO_FAR_AWAY:problem==3?PlayerBedEnterEvent.BedEnterResult.NOT_SAFE:problem==4?PlayerBedEnterEvent.BedEnterResult.NOT_POSSIBLE_HERE:PlayerBedEnterEvent.BedEnterResult.OTHER_PROBLEM;
  PlayerBedEnterEvent event=new PlayerBedEnterEvent(player,bed,result,action);RuntimeState.fire(event);return event.useBed()==org.bukkit.event.Event.Result.DENY;
 }
}
