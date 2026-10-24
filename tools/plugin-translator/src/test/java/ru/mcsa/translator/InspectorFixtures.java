package ru.mcsa.translator;
import java.nio.file.*;
import java.util.jar.*;
import org.objectweb.asm.*;
public final class InspectorFixtures implements Opcodes {
 public static void main(String[] args)throws Exception{
  Path root=Path.of(args[0]);Files.createDirectories(root);
  for(String kind:new String[]{"missing-method","missing-overload","missing-field","nms","reflection","threads","bridge","inherited","generic-signature","adventure-overload","adventure-spoof","executor","parallel-map","atomic-field-thread","unit-sleep","permissions-invalid","alias-core"}){
   String className=kind.equals("adventure-spoof")?"net/kyori/adventure/text/Evil":"fixture/Rejected";
   ClassWriter c=new ClassWriter(0);c.visit(V11,ACC_PUBLIC,className,null,"org/bukkit/plugin/java/JavaPlugin",null);
   if(kind.equals("generic-signature"))c.visitField(ACC_PRIVATE,"hidden","Ljava/util/List;","Ljava/util/List<Lnet/minecraft/server/Forbidden;>;",null).visitEnd();
   if(kind.equals("atomic-field-thread"))c.visitField(ACC_PRIVATE,"hidden","Ljava/util/concurrent/atomic/AtomicReference;","Ljava/util/concurrent/atomic/AtomicReference<Ljava/lang/Thread;>;",null).visitEnd();
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
    case "adventure-overload":m.visitMethodInsn(INVOKESTATIC,"net/kyori/adventure/text/Component","text","(Ljava/io/File;)Lnet/kyori/adventure/text/TextComponent;",true);break;
    case "executor":m.visitMethodInsn(INVOKESTATIC,"java/util/concurrent/Executors","newCachedThreadPool","()Ljava/util/concurrent/ExecutorService;",false);break;
    case "unit-sleep":m.visitMethodInsn(INVOKEVIRTUAL,"java/util/concurrent/TimeUnit","sleep","(J)V",false);break;
    case "parallel-map":m.visitMethodInsn(INVOKEVIRTUAL,"java/util/concurrent/ConcurrentHashMap","forEach","(JLjava/util/function/BiConsumer;)V",false);break;
   }
   m.visitInsn(RETURN);m.visitMaxs(4,1);m.visitEnd();c.visitEnd();
   try(JarOutputStream jar=new JarOutputStream(Files.newOutputStream(root.resolve(kind+".jar")))){
    jar.putNextEntry(new JarEntry(className+".class"));jar.write(c.toByteArray());jar.closeEntry();
    jar.putNextEntry(new JarEntry("plugin.yml"));jar.write(("name: rejected\nversion: '1'\nmain: "+className.replace('/','.')+"\n"+(kind.equals("permissions-invalid")?"permissions:\n  invalid:\n    default: superuser\n":kind.equals("alias-core")?"commands:\n  custom:\n    aliases: [op]\n":"")).getBytes(java.nio.charset.StandardCharsets.UTF_8));jar.closeEntry();
   }
  }
 }
}
