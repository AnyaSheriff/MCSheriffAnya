package ru.mcsa.translator;
import java.nio.file.*;
import java.util.jar.*;
import org.objectweb.asm.*;
public final class MetadataGuardFixtures implements Opcodes {
 public static void main(String[] args)throws Exception{
  Path root=Path.of(args[0]);Files.createDirectories(root);
  for(String kind:new String[]{"supported","overload","lore","legacy-name"}){
   ClassWriter c=new ClassWriter(0);c.visit(V11,ACC_PUBLIC,"fixtures/MetadataGuards",null,"org/bukkit/plugin/java/JavaPlugin",null);
   MethodVisitor m=c.visitMethod(ACC_PUBLIC,"check","(Lorg/bukkit/inventory/meta/ItemMeta;Lorg/bukkit/enchantments/Enchantment;)V",null,null);m.visitCode();
   if(kind.equals("supported")){
    for(String name:new String[]{"hasDisplayName","hasEnchants","hasEnchant","getEnchantLevel"}){
     m.visitVarInsn(ALOAD,1);boolean argument=name.equals("hasEnchant")||name.equals("getEnchantLevel");if(argument)m.visitVarInsn(ALOAD,2);
     m.visitMethodInsn(INVOKEINTERFACE,"org/bukkit/inventory/meta/ItemMeta",name,argument?"(Lorg/bukkit/enchantments/Enchantment;)"+(name.equals("getEnchantLevel")?"I":"Z"):"()Z",true);m.visitInsn(POP);
    }
   }else{
    m.visitVarInsn(ALOAD,1);if(kind.equals("overload"))m.visitLdcInsn("sharpness");
    m.visitMethodInsn(INVOKEINTERFACE,"org/bukkit/inventory/meta/ItemMeta",kind.equals("overload")?"hasEnchant":kind.equals("lore")?"hasLore":"getDisplayName",kind.equals("overload")?"(Ljava/lang/String;)Z":kind.equals("lore")?"()Z":"()Ljava/lang/String;",true);m.visitInsn(POP);
   }
   m.visitInsn(RETURN);m.visitMaxs(2,3);m.visitEnd();c.visitEnd();
   try(JarOutputStream jar=new JarOutputStream(Files.newOutputStream(root.resolve(kind+".jar")))){
    jar.putNextEntry(new JarEntry("fixtures/MetadataGuards.class"));jar.write(c.toByteArray());jar.closeEntry();
    jar.putNextEntry(new JarEntry("plugin.yml"));jar.write("name: metadata-guards\nversion: '1'\nmain: fixtures.MetadataGuards\n".getBytes(java.nio.charset.StandardCharsets.UTF_8));jar.closeEntry();
   }
  }
 }
}
