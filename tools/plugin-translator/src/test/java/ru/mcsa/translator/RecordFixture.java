package ru.mcsa.translator;
// Собственный тестовый record: код входного плагина на JVM не запускается.
public final class RecordFixture {
 public record Value(int number,String text,double zero) {}
 public static void main(String[] args){Value a=new Value(7,"x",-0.0),b=new Value(7,"x",-0.0),c=new Value(7,"x",0.0);if(!a.equals(b)||a.equals(c)||a.equals(null)||a.equals("x")||a.hashCode()!=b.hashCode())throw new AssertionError();System.out.println(a);System.out.println(a.hashCode());}
}
