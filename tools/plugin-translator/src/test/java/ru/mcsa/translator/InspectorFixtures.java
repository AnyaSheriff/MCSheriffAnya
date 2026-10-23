package ru.mcsa.translator;
import java.nio.file.*;
import java.util.jar.*;
import org.objectweb.asm.*;
public final class InspectorFixtures implements Opcodes {
 public static void main(String[] args)throws Exception{
  Path root=Path.of(args[0]);Files.createDirectories(root);
  for(String kind:new String[]{"missing-method","missing-overload","missing-field","nms","reflection","threads","bridge","inherited","generic-signature"}){
   ClassWriter c=new ClassWriter(0);c.visit(V11,ACC_PUBLIC,"fixture/Rejected",null,"org/bukkit/plugin/java/JavaPlugin",null);
   if(kind.equals("generic-signature"))c.visitField(ACC_PRIVATE,"hidden","Ljava/util/List;","Ljava/util/List<Lnet/minecraft/server/Forbidden;>;",null).visitEnd();
   MethodVisitor m=c.visitMethod(ACC_PUBLIC,"onEnable","()V",null,null);m.visitCode();
   switch(kind){
    case "missing-method":m.visitMethodInsn(INVOKESTATIC,"org/bukkit/Bukkit","getUnsupported","()V",false);break;
    case "missing-overload":m.visitMethodInsn(INVOKEINTERFACE,"org/bukkit/entity/Player","sendMessage","(I)V",true);break;
    case "missing-field":m.visitFieldInsn(GETSTATIC,"org/bukkit/Material","UNSUPPORTED_MATERIAL","Lorg/bukkit/Material;");break;
    case "nms":m.visitLdcInsn(Type.getObjectType("net/minecraft/server/Forbidden"));break;
    case "reflection":m.visitMethodInsn(INVOKESTATIC,"java/lang/Class","forName","(Ljava/lang/String;)Ljava/lang/Class;",false);break;
    case "threads":m.visitTypeInsn(NEW,"java/lang/Thread");break;
    case "bridge":m.visitMethodInsn(INVOKESTATIC,"ru/mcsa/bukkit/Host","text","(I[Ljava/lang/String;)Ljava/lang/String;",false);break;
    case "inherited":m.visitMethodInsn(INVOKEVIRTUAL,"fixture/Rejected","getUnsupported","()V",false);break;
   }
   m.visitInsn(RETURN);m.visitMaxs(4,1);m.visitEnd();c.visitEnd();
   try(JarOutputStream jar=new JarOutputStream(Files.newOutputStream(root.resolve(kind+".jar")))){
    jar.putNextEntry(new JarEntry("fixture/Rejected.class"));jar.write(c.toByteArray());jar.closeEntry();
    jar.putNextEntry(new JarEntry("plugin.yml"));jar.write("name: rejected\nversion: '1'\nmain: fixture.Rejected\n".getBytes(java.nio.charset.StandardCharsets.UTF_8));jar.closeEntry();
   }
  }
 }
}
