package org.bukkit.attribute;
public final class AttributeModifier implements org.bukkit.Keyed {
 public enum Operation {ADD_NUMBER,ADD_SCALAR,MULTIPLY_SCALAR_1}
 private final org.bukkit.NamespacedKey key;private final double amount;private final Operation operation;
 public AttributeModifier(org.bukkit.NamespacedKey key,double amount,Operation operation){this.key=java.util.Objects.requireNonNull(key);if(!Double.isFinite(amount))throw new IllegalArgumentException("Модификатор должен быть конечным");this.amount=amount;this.operation=java.util.Objects.requireNonNull(operation);}
 public org.bukkit.NamespacedKey getKey(){return key;}public double getAmount(){return amount;}public Operation getOperation(){return operation;}
}
