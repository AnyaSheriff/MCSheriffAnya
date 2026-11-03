package org.bukkit.event;
@java.lang.annotation.Retention(java.lang.annotation.RetentionPolicy.RUNTIME) @java.lang.annotation.Target(java.lang.annotation.ElementType.METHOD)
public @interface EventHandler { EventPriority priority() default EventPriority.NORMAL; boolean ignoreCancelled() default false; }
