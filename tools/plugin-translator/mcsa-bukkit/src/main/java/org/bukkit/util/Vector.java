package org.bukkit.util;
public class Vector implements Cloneable {
 private double x,y,z;public Vector(){this(0,0,0);}public Vector(double x,double y,double z){this.x=x;this.y=y;this.z=z;}public Vector(int x,int y,int z){this((double)x,y,z);}public Vector(float x,float y,float z){this((double)x,y,z);}
 public double getX(){return x;}public double getY(){return y;}public double getZ(){return z;}
 public Vector setX(double value){x=value;return this;}public Vector setY(double value){y=value;return this;}public Vector setZ(double value){z=value;return this;}
 public Vector setX(int value){return setX((double)value);}public Vector setY(int value){return setY((double)value);}public Vector setZ(int value){return setZ((double)value);}
 public double lengthSquared(){return x*x+y*y+z*z;}public double length(){return Math.sqrt(lengthSquared());}public double dot(Vector other){return x*other.x+y*other.y+z*other.z;}
 public Vector multiply(double factor){x*=factor;y*=factor;z*=factor;return this;}public Vector normalize(){return multiply(1.0/length());}
 public Vector add(Vector v){x+=v.x;y+=v.y;z+=v.z;return this;}public Vector subtract(Vector v){x-=v.x;y-=v.y;z-=v.z;return this;}
 public double distanceSquared(Vector v){double a=x-v.x,b=y-v.y,c=z-v.z;return a*a+b*b+c*c;}public double distance(Vector v){return Math.sqrt(distanceSquared(v));}
 public Vector clone(){return new Vector(x,y,z);}
}
