package ru.mcsa.translator;
import java.nio.file.*;
import java.util.jar.*;
import org.objectweb.asm.*;
public final class HandlerFixtures implements Opcodes {
 public static void main(String[] args)throws Exception{
  Path root=Path.of(args[0]);Files.createDirectories(root);
  for(String kind:new String[]{"listener","global","plugin"}){
   ClassWriter c=new ClassWriter(0);c.visit(V11,ACC_PUBLIC,"fixture/Handlers",null,"org/bukkit/plugin/java/JavaPlugin",null);
   MethodVisitor m=c.visitMethod(ACC_PUBLIC,"onEnable","()V",null,null);m.visitCode();
   String descriptor=kind.equals("global")?"()V":kind.equals("plugin")?"(Lorg/bukkit/plugin/Plugin;)V":"(Lorg/bukkit/event/Listener;)V";
   if(!kind.equals("global"))m.visitInsn(ACONST_NULL);
   m.visitMethodInsn(INVOKESTATIC,"org/bukkit/event/HandlerList","unregisterAll",descriptor,false);
   m.visitInsn(RETURN);m.visitMaxs(1,1);m.visitEnd();c.visitEnd();
   try(JarOutputStream jar=new JarOutputStream(Files.newOutputStream(root.resolve(kind+".jar")))){
    jar.putNextEntry(new JarEntry("fixture/Handlers.class"));jar.write(c.toByteArray());jar.closeEntry();
    jar.putNextEntry(new JarEntry("plugin.yml"));jar.write("name: handlers\nversion: '1'\nmain: fixture.Handlers\n".getBytes(java.nio.charset.StandardCharsets.UTF_8));jar.closeEntry();
   }
  }
 }
}
