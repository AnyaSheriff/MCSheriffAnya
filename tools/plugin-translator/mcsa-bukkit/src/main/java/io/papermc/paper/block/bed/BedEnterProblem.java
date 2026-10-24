package io.papermc.paper.block.bed;
import net.kyori.adventure.text.Component;
public interface BedEnterProblem {
 BedEnterProblem TOO_FAR_AWAY=()->Component.text("Кровать слишком далеко"),OBSTRUCTED=()->Component.text("Кровать загорожена"),NOT_SAFE=()->Component.text("Рядом чудовища"),EXPLOSION=()->null,OTHER=()->null;
 Component errorMessage();
}
