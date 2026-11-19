import ru.mcsa.modapi.Mod;
import ru.mcsa.modapi.Context;

public final class ChatAudit implements Mod {
    public void initialize(Context ctx) {
        ctx.onChat(chat -> {
            ctx.broadcast("chat:first:" + chat.playerName() + ":" + chat.message());
            chat.cancel();
        });
        ctx.onChat(chat -> {
            ctx.broadcast("chat:second:" + chat.playerUuid() + ":" + chat.message());
            chat.cancel();
        });
    }
}
