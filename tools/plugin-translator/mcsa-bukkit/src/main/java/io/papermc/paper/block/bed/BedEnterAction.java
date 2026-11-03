package io.papermc.paper.block.bed;
public interface BedEnterAction {BedRuleResult canSetSpawn();BedRuleResult canSleep();BedEnterProblem problem();net.kyori.adventure.text.Component errorMessage();}
