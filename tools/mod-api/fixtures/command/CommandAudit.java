import ru.mcsa.modapi.Mod;
import ru.mcsa.modapi.Context;

public final class CommandAudit implements Mod {
    public void initialize(Context ctx) {
        ctx.onCommand("mod_echo", 0, command -> {
            ctx.reply(command, command.name() + ":" + command.senderName() + ":" + command.argument(0) + ":" + command.argument(1));
            ctx.log("command fixture ran");
        });
        ctx.onCommand("mod_admin", 4, command -> {
            ctx.reply(command, "operator-only");
        });
    }
}
