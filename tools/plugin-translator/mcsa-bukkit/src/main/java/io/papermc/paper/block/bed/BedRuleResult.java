package io.papermc.paper.block.bed;
public interface BedRuleResult {BedRuleResult ALLOWED=()->true,NEVER=()->false,TOO_MUCH_LIGHT=()->false;boolean success();}
