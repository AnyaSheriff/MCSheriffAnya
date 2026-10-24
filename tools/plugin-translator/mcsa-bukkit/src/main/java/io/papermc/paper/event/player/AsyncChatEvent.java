package io.papermc.paper.event.player;
import net.kyori.adventure.text.Component;import net.kyori.adventure.audience.Audience;import io.papermc.paper.chat.ChatRenderer;import java.util.*;
// Обработчики компонента последовательны, даже если событие пришло с сетевого потока.
public final class AsyncChatEvent extends org.bukkit.event.player.PlayerEvent implements org.bukkit.event.Cancellable {
 private final Component original;private Component message;private ChatRenderer renderer;private boolean cancelled;
 private final Set<Audience> viewers=new LinkedHashSet<>(),initialViewers=new LinkedHashSet<>();
 public AsyncChatEvent(org.bukkit.entity.Player player,Component message){super(player);this.original=message;this.message=message;viewers.addAll(org.bukkit.Bukkit.getOnlinePlayers());initialViewers.addAll(viewers);}
 public Component message(){return message;}public void message(Component value){message=Objects.requireNonNull(value);}public Component originalMessage(){return original;}
 public ChatRenderer renderer(){return renderer==null?ChatRenderer.defaultRenderer():renderer;}public void renderer(ChatRenderer value){renderer=Objects.requireNonNull(value);}
 public Set<Audience> viewers(){return viewers;}public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
 public boolean hasCustomDelivery(){return renderer!=null||!message.equals(original)||!viewers.equals(initialViewers);}
 public void deliver(){Component name=Component.text(getPlayer().getName());for(Audience viewer:new ArrayList<>(viewers))viewer.sendMessage(renderer().render(getPlayer(),name,message,viewer));}
}
