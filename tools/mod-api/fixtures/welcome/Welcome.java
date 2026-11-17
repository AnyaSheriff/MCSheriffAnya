import ru.mcsa.modapi.Mod;
import ru.mcsa.modapi.Context;

public final class Welcome implements Mod {
    public void initialize(Context ctx) {
        ctx.onStart(() -> {
            ctx.log("mod-fixture started");
            ctx.broadcast("mod:start");
        });
        ctx.onJoin(player -> {
            ctx.message(player, "mod:join:1:" + player.name());
        });
        ctx.onJoin(player -> {
            ctx.message(player, "mod:join:2:" + player.uuid());
        });
        ctx.onQuit(player -> {
            ctx.broadcast("mod:quit:" + player.name());
        });
        ctx.every(2, () -> {
            ctx.broadcast("mod:periodic");
        });
        ctx.onStop(() -> {
            ctx.broadcast("mod:stop");
        });
    }
}
