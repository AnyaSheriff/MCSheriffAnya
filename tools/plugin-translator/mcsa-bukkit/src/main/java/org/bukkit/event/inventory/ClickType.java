package org.bukkit.event.inventory;
public enum ClickType { LEFT,SHIFT_LEFT,RIGHT,SHIFT_RIGHT,WINDOW_BORDER_LEFT,WINDOW_BORDER_RIGHT,MIDDLE,NUMBER_KEY,DOUBLE_CLICK,DROP,CONTROL_DROP,CREATIVE,SWAP_OFFHAND,UNKNOWN;
 public boolean isLeftClick(){return this==LEFT||this==SHIFT_LEFT||this==DOUBLE_CLICK||this==CREATIVE;}
 public boolean isRightClick(){return this==RIGHT||this==SHIFT_RIGHT;}
 public boolean isShiftClick(){return this==SHIFT_LEFT||this==SHIFT_RIGHT;}
}
