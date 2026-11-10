package ru.mcsa.bukkit;
import java.util.HashMap;
import java.util.Map;
import org.bukkit.event.block.BlockBreakEvent;

/** Внутренний native frame; не является глобальным PluginManager.callEvent. */
final class PriorityFrames {
 private static final int CAPACITY=32;
 private static final Map<Long,Frame> frames=new HashMap<>();
 private static long last;
 // C-мост передаёт полный u64 как 16 hex цифр; без отсутствующего TeaVM parseUnsignedLong.
 private static long parseId(String text){if(text.length()!=16)throw new IllegalArgumentException("native id width");long result=0;for(int i=0;i<16;i++){char c=text.charAt(i);int digit=c>='0'&&c<='9'?c-'0':c>='a'&&c<='f'?c-'a'+10:-1;if(digit<0)throw new IllegalArgumentException("native id digit");result=(result<<4)|digit;}return result;}
 private static final class Frame {final BlockBreakEvent event;int next;Frame(BlockBreakEvent event){this.event=event;}}
 static int begin(String[] a){
  long id=parseId(a[0]);
  if(id==0)return -1;
  if(frames.containsKey(id))return -2;
  if((id^Long.MIN_VALUE)<=(last^Long.MIN_VALUE))return -4;
  if(frames.size()>=CAPACITY)return -3;
  org.bukkit.entity.Player player=Host.player(a,2);
  org.bukkit.block.Block block=new RemoteBlock(new RemoteWorld(Integer.parseInt(a[11])),Integer.parseInt(a[12]),Integer.parseInt(a[13]),Integer.parseInt(a[14]));
  BlockBreakEvent event=new BlockBreakEvent(block,player);event.setCancelled(a[1].equals("1"));
  frames.put(id,new Frame(event));last=id;return 0;
 }
 static int phase(String[] a){
  long id=parseId(a[0]);int priority=Integer.parseInt(a[1]);
  Frame frame=frames.get(id);if(frame==null)return -1;
  if(priority<0||priority>5)return -5;
  if(priority!=frame.next)return -4;
  frame.event.setCancelled(a[2].equals("1"));
  // Продвижение перед user callback не даёт повторно вызвать эту phase через мост.
  frame.next++;RuntimeState.firePriority(frame.event,priority);
  return frame.event.isCancelled()?1:0;
 }
 static int finish(String[] a){
  long id=parseId(a[0]);Frame frame=frames.get(id);if(frame==null)return -1;
  if(frame.next!=6)return -4;
  frame.event.setCancelled(a[1].equals("1"));frames.remove(id);return 0;
 }
 static void clear(){frames.clear();}
}
