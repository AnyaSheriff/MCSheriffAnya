package ru.mcsa.translator;
import java.io.File;
import java.net.*;
import org.teavm.tooling.*;
public final class Compile {
 public static void main(String[] args)throws Exception{
  TeaVMTool tool=new TeaVMTool();tool.setTargetType(TeaVMTargetType.C);tool.setMainClass("mcsa.generated.Main");tool.setTargetDirectory(new File(args[0]));tool.setTargetFileName("plugin.c");
  tool.setMinHeapSize(2<<20);tool.setMaxHeapSize(16<<20);tool.setLog(new ConsoleTeaVMToolLog(false));
  URL[] urls=new URL[args.length-1];for(int i=1;i<args.length;i++)urls[i-1]=new File(args[i]).toURI().toURL();
  try(URLClassLoader loader=new URLClassLoader(urls,Compile.class.getClassLoader())){
   tool.setClassLoader(loader);tool.generate();TeaVMProblemRenderer.describeProblems(tool.getDependencyInfo().getCallGraph(),tool.getProblemProvider(),new ConsoleTeaVMToolLog(false));
   if(!tool.getProblemProvider().getSevereProblems().isEmpty())throw new IllegalStateException("TeaVM: перевод отклонён из-за ошибок");
  }
 }
}
