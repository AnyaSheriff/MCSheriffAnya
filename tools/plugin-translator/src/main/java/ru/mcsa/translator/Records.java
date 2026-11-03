package ru.mcsa.translator;
import org.objectweb.asm.*;import org.objectweb.asm.tree.*;

// Стандартные методы Java record раскрываются без рефлексии ObjectMethods.
final class Records implements Opcodes {
 static void lower(ClassNode c){
  if(!"java/lang/Record".equals(c.superName)||c.recordComponents==null||(c.access&ACC_FINAL)==0)return;
  for(MethodNode method:c.methods){boolean bootstrap=false;for(AbstractInsnNode i:method.instructions)if(i instanceof InvokeDynamicInsnNode&&((InvokeDynamicInsnNode)i).bsm.getOwner().equals("java/lang/runtime/ObjectMethods"))bootstrap=true;if(!bootstrap)continue;
   MethodNode replacement=new MethodNode(ASM9,method.access,method.name,method.desc,null,null);
   if(method.name.equals("equals")&&method.desc.equals("(Ljava/lang/Object;)Z"))equals(c,replacement);
   else if(method.name.equals("hashCode")&&method.desc.equals("()I"))hashCode(c,replacement);
   else if(method.name.equals("toString")&&method.desc.equals("()Ljava/lang/String;"))toString(c,replacement);
   else continue;
   method.instructions=replacement.instructions;method.tryCatchBlocks.clear();method.localVariables=null;method.maxStack=8;method.maxLocals=3;
  }
  for(MethodNode m:c.methods)for(AbstractInsnNode i:m.instructions)if(i instanceof MethodInsnNode){MethodInsnNode call=(MethodInsnNode)i;if(call.owner.equals("java/lang/Record")&&call.name.equals("<init>"))call.owner="java/lang/Object";}
  c.superName="java/lang/Object";c.access&=~ACC_RECORD;c.recordComponents=null;
 }
 private static void field(ClassNode c,RecordComponentNode field,MethodVisitor m,int local){m.visitVarInsn(ALOAD,local);m.visitFieldInsn(GETFIELD,c.name,field.name,field.descriptor);}
 private static void equals(ClassNode c,MethodVisitor m){Label fail=new Label();m.visitVarInsn(ALOAD,1);m.visitTypeInsn(INSTANCEOF,c.name);m.visitJumpInsn(IFEQ,fail);m.visitVarInsn(ALOAD,1);m.visitTypeInsn(CHECKCAST,c.name);m.visitVarInsn(ASTORE,2);
  for(RecordComponentNode f:c.recordComponents){field(c,f,m,0);field(c,f,m,2);switch(Type.getType(f.descriptor).getSort()){
   case Type.BOOLEAN:case Type.BYTE:case Type.CHAR:case Type.SHORT:case Type.INT:m.visitJumpInsn(IF_ICMPNE,fail);break;
   case Type.LONG:m.visitInsn(LCMP);m.visitJumpInsn(IFNE,fail);break;
   case Type.FLOAT:m.visitMethodInsn(INVOKESTATIC,"java/lang/Float","compare","(FF)I",false);m.visitJumpInsn(IFNE,fail);break;
   case Type.DOUBLE:m.visitMethodInsn(INVOKESTATIC,"java/lang/Double","compare","(DD)I",false);m.visitJumpInsn(IFNE,fail);break;
   default:m.visitMethodInsn(INVOKESTATIC,"java/util/Objects","equals","(Ljava/lang/Object;Ljava/lang/Object;)Z",false);m.visitJumpInsn(IFEQ,fail);
  }}m.visitInsn(ICONST_1);m.visitInsn(IRETURN);m.visitLabel(fail);m.visitFrame(F_SAME,0,null,0,null);m.visitInsn(ICONST_0);m.visitInsn(IRETURN);
 }
 private static void hashCode(ClassNode c,MethodVisitor m){m.visitInsn(ICONST_0);m.visitVarInsn(ISTORE,1);for(RecordComponentNode f:c.recordComponents){m.visitVarInsn(ILOAD,1);m.visitIntInsn(BIPUSH,31);m.visitInsn(IMUL);field(c,f,m,0);Type type=Type.getType(f.descriptor);String owner,desc;switch(type.getSort()){
  case Type.BOOLEAN:owner="Boolean";desc="Z";break;case Type.BYTE:case Type.SHORT:case Type.INT:owner="Integer";desc="I";break;case Type.CHAR:owner="Character";desc="C";break;case Type.LONG:owner="Long";desc="J";break;case Type.FLOAT:owner="Float";desc="F";break;case Type.DOUBLE:owner="Double";desc="D";break;default:owner=null;desc="Ljava/lang/Object;";
 }m.visitMethodInsn(INVOKESTATIC,owner==null?"java/util/Objects":"java/lang/"+owner,"hashCode","("+desc+")I",false);m.visitInsn(IADD);m.visitVarInsn(ISTORE,1);}m.visitVarInsn(ILOAD,1);m.visitInsn(IRETURN);}
 private static void append(MethodVisitor m,String text){m.visitLdcInsn(text);m.visitMethodInsn(INVOKEVIRTUAL,"java/lang/StringBuilder","append","(Ljava/lang/String;)Ljava/lang/StringBuilder;",false);}
 private static void toString(ClassNode c,MethodVisitor m){m.visitTypeInsn(NEW,"java/lang/StringBuilder");m.visitInsn(DUP);m.visitMethodInsn(INVOKESPECIAL,"java/lang/StringBuilder","<init>","()V",false);String name=c.name.substring(c.name.lastIndexOf('/')+1);name=name.substring(name.lastIndexOf('$')+1);append(m,name+"[");boolean first=true;for(RecordComponentNode f:c.recordComponents){append(m,(first?"":", ")+f.name+"=");first=false;field(c,f,m,0);Type t=Type.getType(f.descriptor);String desc=t.getSort()==Type.OBJECT||t.getSort()==Type.ARRAY?"Ljava/lang/Object;":t.getSort()==Type.BYTE||t.getSort()==Type.SHORT?"I":f.descriptor;m.visitMethodInsn(INVOKEVIRTUAL,"java/lang/StringBuilder","append","("+desc+")Ljava/lang/StringBuilder;",false);}append(m,"]");m.visitMethodInsn(INVOKEVIRTUAL,"java/lang/StringBuilder","toString","()Ljava/lang/String;",false);m.visitInsn(ARETURN);}
}
