package ru.mcsa.translator;
import java.nio.file.*;
import java.util.*;
import java.util.jar.*;
import org.objectweb.asm.*;
import org.objectweb.asm.tree.*;
import org.objectweb.asm.commons.*;

// Только известная фабрика очереди; произвольный Thread остаётся видимым проверке.
public final class Normalize {
 private static AbstractInsnNode next(AbstractInsnNode i){do{i=i.getNext();}while(i!=null&&i.getOpcode()<0);return i;}
 private static final Set<String> MAPPED=Set.of("Executor","ExecutorService","Executors","Future","Callable");
 static String mapped(String name){String prefix="java/util/concurrent/";if(name.startsWith(prefix)&&Set.of("RejectedExecutionException","ExecutionException","CancellationException","TimeoutException").contains(name.substring(prefix.length())))return "ru/mcsa/bukkit/errors/"+name.substring(prefix.length());if(name.startsWith(prefix)&&MAPPED.contains(name.substring(prefix.length())))return "ru/mcsa/bukkit/concurrent/"+name.substring(prefix.length());if(name.equals("java/util/logging/Logger"))return "ru/mcsa/bukkit/logging/Logger";if(name.equals("java/io/File"))return "ru/mcsa/bukkit/fs/File";if(Set.of("Path","Files","CopyOption","StandardCopyOption","LinkOption","OpenOption","AtomicMoveNotSupportedException").stream().anyMatch(value->name.equals("java/nio/file/"+value)))return "ru/mcsa/bukkit/fs/"+name.substring("java/nio/file/".length());if(name.equals("java/nio/file/attribute/FileAttribute"))return "ru/mcsa/bukkit/fs/FileAttribute";return name;}
 static String mappedDescriptor(String desc){return new Remapper(){@Override public String map(String name){return mapped(name);}}.mapMethodDesc(desc);}
 private static byte[] transform(byte[] bytes,boolean remap){
  ClassNode c=new ClassNode();new ClassReader(bytes).accept(c,0);
  if(!remap){
   Records.lower(c);
   Set<MethodNode> unused=new HashSet<>();
   for(MethodNode m:c.methods){
    // Монитор метода не нужен в однопоточном компоненте, native не разрешаем.
    m.access&=~Opcodes.ACC_SYNCHRONIZED;
    for(AbstractInsnNode i:m.instructions.toArray()){
     if(i instanceof InvokeDynamicInsnNode){InvokeDynamicInsnNode d=(InvokeDynamicInsnNode)i;AbstractInsnNode n=next(i);
      if(Type.getReturnType(d.desc).equals(Type.getObjectType("java/util/concurrent/ThreadFactory"))&&d.bsm.getOwner().equals("java/lang/invoke/LambdaMetafactory")&&n instanceof MethodInsnNode){MethodInsnNode call=(MethodInsnNode)n;
       if(call.getOpcode()==Opcodes.INVOKESTATIC&&call.owner.equals("java/util/concurrent/Executors")&&((call.name.equals("newSingleThreadExecutor")&&call.desc.equals("(Ljava/util/concurrent/ThreadFactory;)Ljava/util/concurrent/ExecutorService;"))||(call.name.equals("newFixedThreadPool")&&call.desc.equals("(ILjava/util/concurrent/ThreadFactory;)Ljava/util/concurrent/ExecutorService;")))&&d.bsmArgs.length>1&&d.bsmArgs[1] instanceof Handle){
        Handle impl=(Handle)d.bsmArgs[1];
        if(impl.getOwner().equals(c.name))for(MethodNode factory:c.methods)if(factory.name.equals(impl.getName())&&factory.desc.equals(impl.getDesc())&&(factory.access&Opcodes.ACC_SYNTHETIC)!=0){
         // Удаляем только фабрику без других ссылок во всём классе.
         int refs=0;for(MethodNode method:c.methods)for(AbstractInsnNode insn:method.instructions){if(insn instanceof MethodInsnNode){MethodInsnNode r=(MethodInsnNode)insn;if(r.owner.equals(c.name)&&r.name.equals(factory.name)&&r.desc.equals(factory.desc))refs++;}else if(insn instanceof InvokeDynamicInsnNode)for(Object arg:((InvokeDynamicInsnNode)insn).bsmArgs)if(arg instanceof Handle&&((Handle)arg).getOwner().equals(c.name)&&((Handle)arg).getName().equals(factory.name)&&((Handle)arg).getDesc().equals(factory.desc))refs++;}
         if(refs==1&&(factory.access&Opcodes.ACC_PRIVATE)!=0){unused.add(factory);InsnList drop=new InsnList();Type[] captures=Type.getArgumentTypes(d.desc);for(int index=captures.length-1;index>=0;index--)drop.add(new InsnNode(captures[index].getSize()==2?Opcodes.POP2:Opcodes.POP));m.instructions.insertBefore(i,drop);m.instructions.remove(i);call.desc=call.name.equals("newFixedThreadPool")?"(I)Ljava/util/concurrent/ExecutorService;":"()Ljava/util/concurrent/ExecutorService;";}
        }
       }
      }
     }else if(i instanceof MethodInsnNode){MethodInsnNode a=(MethodInsnNode)i;AbstractInsnNode n=next(i);
      if(a.owner.equals("java/lang/Thread")&&a.name.equals("currentThread")&&a.desc.equals("()Ljava/lang/Thread;")&&n instanceof MethodInsnNode){MethodInsnNode b=(MethodInsnNode)n;if(b.owner.equals("java/lang/Thread")&&b.name.equals("interrupt")&&b.desc.equals("()V")){m.instructions.remove(n);m.instructions.remove(i);}}
     }
    }
   }
   c.methods.removeAll(unused);
  }
  ClassWriter out=new ClassWriter(0);
  if(remap)c.accept(new ClassRemapper(out,new Remapper(){@Override public String map(String name){return mapped(name);}}));else c.accept(out);
  return out.toByteArray();
 }
 public static void main(String[] args)throws Exception{
  try(JarFile in=new JarFile(args[0]);JarOutputStream out=new JarOutputStream(Files.newOutputStream(Path.of(args[1])))){
   Enumeration<JarEntry> entries=in.entries();Set<String> names=new HashSet<>();long total=0;
   while(entries.hasMoreElements()){JarEntry e=entries.nextElement();if(e.isDirectory())continue;if(!names.add(e.getName()))throw new IllegalArgumentException("Повтор записи JAR");byte[] bytes=in.getInputStream(e).readNBytes(8*1024*1024+1);total+=bytes.length;if(bytes.length>8*1024*1024||total>64*1024*1024)throw new IllegalArgumentException("Превышен размер JAR");if(e.getName().endsWith(".class"))bytes=transform(bytes,args.length>2);out.putNextEntry(new JarEntry(e.getName()));out.write(bytes);out.closeEntry();}
  }
 }
}
