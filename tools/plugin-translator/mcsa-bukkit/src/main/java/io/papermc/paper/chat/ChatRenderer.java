package io.papermc.paper.chat;
import net.kyori.adventure.text.Component;import net.kyori.adventure.audience.Audience;import org.bukkit.entity.Player;
public interface ChatRenderer {
 Component render(Player source,Component displayName,Component message,Audience viewer);
 interface ViewerUnaware {Component render(Player source,Component displayName,Component message);}
 static ChatRenderer viewerUnaware(ViewerUnaware renderer){return (source,name,message,viewer)->renderer.render(source,name,message);}
 static ChatRenderer defaultRenderer(){return (source,name,message,viewer)->Component.text("<").append(name).append(Component.text("> ")).append(message);}
}
