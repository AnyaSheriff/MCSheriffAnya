package ru.mcsa.translator;
import java.nio.file.*;
import java.nio.charset.StandardCharsets;
import java.util.jar.*;
import org.objectweb.asm.*;

// Собственные недостижимые ссылки: gate обязан проверить даже их до TeaVM.
public final class BossBarFixtures implements Opcodes {
 public static void main(String[] args)throws Exception{
  Path root=Path.of(args[0]);Files.createDirectories(root);
  for(String kind:new String[]{"supported","flag-field","flag-method","keyed","wrong-overload"}){
   ClassWriter c=new ClassWriter(0);c.visit(V11,ACC_PUBLIC,"fixtures/BossBarGuard",null,"org/bukkit/plugin/java/JavaPlugin",null);
   MethodVisitor m=c.visitMethod(ACC_PUBLIC,"check","(Lorg/bukkit/boss/BossBar;)V",null,null);m.visitCode();
   if(kind.equals("supported")){
    m.visitVarInsn(ALOAD,1);m.visitLdcInsn(0.25);m.visitMethodInsn(INVOKEINTERFACE,"org/bukkit/boss/BossBar","setProgress","(D)V",true);
    m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKEINTERFACE,"org/bukkit/boss/BossBar","getPlayers","()Ljava/util/List;",true);m.visitInsn(POP);
   }else if(kind.equals("flag-field")){
    m.visitFieldInsn(GETSTATIC,"org/bukkit/boss/BarFlag","DARKEN_SKY","Lorg/bukkit/boss/BarFlag;");m.visitInsn(POP);
   }else if(kind.equals("flag-method")){
    m.visitVarInsn(ALOAD,1);m.visitInsn(ACONST_NULL);m.visitMethodInsn(INVOKEINTERFACE,"org/bukkit/boss/BossBar","addFlag","(Lorg/bukkit/boss/BarFlag;)V",true);
   }else if(kind.equals("keyed")){
    for(int i=0;i<5;i++)m.visitInsn(ACONST_NULL);
    m.visitMethodInsn(INVOKESTATIC,"org/bukkit/Bukkit","createBossBar","(Lorg/bukkit/NamespacedKey;Ljava/lang/String;Lorg/bukkit/boss/BarColor;Lorg/bukkit/boss/BarStyle;[Lorg/bukkit/boss/BarFlag;)Lorg/bukkit/boss/KeyedBossBar;",false);m.visitInsn(POP);
   }else{
    m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKEINTERFACE,"org/bukkit/boss/BossBar","getProgress","()F",true);m.visitInsn(POP);
   }
   m.visitInsn(RETURN);m.visitMaxs(5,2);m.visitEnd();c.visitEnd();
   try(JarOutputStream jar=new JarOutputStream(Files.newOutputStream(root.resolve(kind+".jar")))){
    jar.putNextEntry(new JarEntry("fixtures/BossBarGuard.class"));jar.write(c.toByteArray());jar.closeEntry();
    jar.putNextEntry(new JarEntry("plugin.yml"));jar.write("name: bossbar-guard\nversion: '1'\nmain: fixtures.BossBarGuard\n".getBytes(StandardCharsets.UTF_8));jar.closeEntry();
   }
  }
 }
}
