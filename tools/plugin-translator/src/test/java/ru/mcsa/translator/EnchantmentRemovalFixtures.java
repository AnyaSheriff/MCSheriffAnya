package ru.mcsa.translator;
import java.nio.file.*;
import java.util.jar.*;
import org.objectweb.asm.*;
public final class EnchantmentRemovalFixtures implements Opcodes {
 public static void main(String[] args)throws Exception{
  Path root=Path.of(args[0]);Files.createDirectories(root);
  for(String kind:new String[]{"supported","overload","meta-all","stack-all"}){
   ClassWriter c=new ClassWriter(0);c.visit(V11,ACC_PUBLIC,"fixtures/EnchantmentRemoval",null,"org/bukkit/plugin/java/JavaPlugin",null);
   MethodVisitor m=c.visitMethod(ACC_PUBLIC,"check","(Lorg/bukkit/inventory/meta/ItemMeta;Lorg/bukkit/inventory/ItemStack;Lorg/bukkit/enchantments/Enchantment;)V",null,null);m.visitCode();
   if(kind.equals("supported")){
    m.visitVarInsn(ALOAD,1);m.visitVarInsn(ALOAD,3);m.visitMethodInsn(INVOKEINTERFACE,"org/bukkit/inventory/meta/ItemMeta","removeEnchant","(Lorg/bukkit/enchantments/Enchantment;)Z",true);m.visitInsn(POP);
    m.visitVarInsn(ALOAD,2);m.visitVarInsn(ALOAD,3);m.visitMethodInsn(INVOKEVIRTUAL,"org/bukkit/inventory/ItemStack","removeEnchantment","(Lorg/bukkit/enchantments/Enchantment;)I",false);m.visitInsn(POP);
   }else if(kind.equals("overload")){
    m.visitVarInsn(ALOAD,2);m.visitLdcInsn("sharpness");m.visitMethodInsn(INVOKEVIRTUAL,"org/bukkit/inventory/ItemStack","removeEnchantment","(Ljava/lang/String;)I",false);m.visitInsn(POP);
   }else{
    boolean meta=kind.equals("meta-all");m.visitVarInsn(ALOAD,meta?1:2);m.visitMethodInsn(meta?INVOKEINTERFACE:INVOKEVIRTUAL,meta?"org/bukkit/inventory/meta/ItemMeta":"org/bukkit/inventory/ItemStack","removeEnchantments","()V",meta);
   }
   m.visitInsn(RETURN);m.visitMaxs(2,4);m.visitEnd();c.visitEnd();
   try(JarOutputStream jar=new JarOutputStream(Files.newOutputStream(root.resolve(kind+".jar")))){
    jar.putNextEntry(new JarEntry("fixtures/EnchantmentRemoval.class"));jar.write(c.toByteArray());jar.closeEntry();
    jar.putNextEntry(new JarEntry("plugin.yml"));jar.write("name: enchantment-removal\nversion: '1'\nmain: fixtures.EnchantmentRemoval\n".getBytes(java.nio.charset.StandardCharsets.UTF_8));jar.closeEntry();
   }
  }
 }
}
