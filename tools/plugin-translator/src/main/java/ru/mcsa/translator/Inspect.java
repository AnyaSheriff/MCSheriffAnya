package ru.mcsa.translator;
import java.io.*;
import java.nio.file.*;
import java.util.*;
import java.util.jar.*;
import org.objectweb.asm.*;
import org.objectweb.asm.tree.*;
import org.objectweb.asm.signature.*;
import org.yaml.snakeyaml.*;
import org.yaml.snakeyaml.constructor.SafeConstructor;
public final class Inspect {
 private static final Set<String> CORE_COMMANDS=Set.of("auth","changepassword","op","deop","stop","plugman","gamemode","give","tp","teleport","help","list","say","me","msg","tell","w","execute","function","data","setblock","fill","summon","kill","effect","time","weather","gamerule","difficulty","save-all","save-off","save-on","reload","trigger","scoreboard","team","tag","tick","clear","xp","experience","enchant","item","damage","spawnpoint","setworldspawn","worldborder","advancement","attribute","bossbar","clone","datapack","fillbiome","forceload","locate","loot","place","random","return","ride","rotate","schedule","spectate","spreadplayers","teammsg","tm","tnt","tellraw","title","playsound","stopsound","particle","defaultgamemode","seed","version","ban","ban-ip","banlist","pardon","pardon-ip","whitelist","setidletimeout","transfer");
 private Path report;private boolean placeholderAbsent,watchArmor;
 private static final Set<String> MAIN_THREAD_CONCURRENT=Set.of("java/util/concurrent/Executor","java/util/concurrent/ExecutorService","java/util/concurrent/Executors","java/util/concurrent/Future","java/util/concurrent/Callable","java/util/concurrent/ExecutionException","java/util/concurrent/TimeoutException","java/util/concurrent/CancellationException","java/util/concurrent/TimeUnit","java/util/concurrent/RejectedExecutionException","java/util/concurrent/ThreadLocalRandom","java/util/concurrent/ConcurrentMap","java/util/concurrent/ConcurrentHashMap","java/util/concurrent/ConcurrentHashMap$KeySetView","java/util/concurrent/atomic/AtomicBoolean","java/util/concurrent/atomic/AtomicInteger","java/util/concurrent/atomic/AtomicLong","java/util/concurrent/atomic/AtomicReference");
 private static final Set<String> SAFE_IO=Set.of("java/io/IOException","java/io/InputStream","java/io/Reader","java/io/InputStreamReader","java/io/ByteArrayInputStream","java/io/StringReader","java/io/Closeable");
 private boolean api(String name){return name.startsWith("io/papermc/paper/block/bed/")||name.startsWith("io/papermc/paper/chat/")||name.startsWith("io/papermc/paper/event/player/")||name.startsWith("com/destroystokyo/paper/event/player/PlayerArmorChangeEvent")||name.equals("com/destroystokyo/paper/event/player/PlayerJumpEvent")||placeholderAbsent&&name.equals("me/clip/placeholderapi/expansion/PlaceholderExpansion")||name.startsWith("org/mcsa/api/")||name.equals("io/papermc/paper/persistence/PersistentDataContainerView")||name.equals("io/papermc/paper/plugin/configuration/PluginMeta")||name.startsWith("org/bukkit/")||name.startsWith("net/kyori/adventure/");}
 private final Map<String,ClassNode> sdk=new HashMap<>(),classes=new TreeMap<>();
 private final Set<String> errors=new TreeSet<>();private final List<String> handlers=new ArrayList<>();
 private static String quote(String s){StringBuilder b=new StringBuilder("\"");for(char c:s.toCharArray()){if(c=='"'||c=='\\')b.append('\\').append(c);else if(c=='\n')b.append("\\n");else if(c=='\r')b.append("\\r");else if(c=='\t')b.append("\\t");else if(c=='\b')b.append("\\b");else if(c=='\f')b.append("\\f");else if(c<32||c>126)b.append(String.format("\\u%04x",(int)c));else b.append(c);}return b.append('"').toString();}
 private static boolean validNetworkHost(String host){if(host.length()>253||!host.contains(".")||host.matches("[0-9]+(\\.[0-9]+){3}"))return false;for(String part:host.split("\\.",-1))if(!part.matches("[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?"))return false;return true;}
 private void error(String where,String message){errors.add(where+": "+message);}
 private void load(JarFile jar,Map<String,ClassNode> map)throws IOException{
  long total=0;Enumeration<JarEntry> entries=jar.entries();while(entries.hasMoreElements()){JarEntry e=entries.nextElement();if(e.getSize()>8*1024*1024)throw new IOException("Слишком большая запись JAR: "+e.getName());
   if(!e.getName().endsWith(".class"))continue;if(e.getName().startsWith("META-INF/versions/"))throw new IOException("Многоверсионные JAR не поддерживаются");
   byte[] bytes=jar.getInputStream(e).readNBytes(8*1024*1024+1);total+=bytes.length;if(total>64*1024*1024||bytes.length>8*1024*1024)throw new IOException("Превышен размер классов JAR");
   ClassNode n=new ClassNode();ClassReader reader=new ClassReader(bytes);reader.accept(n,0);if(map.put(n.name,n)!=null)throw new IOException("Повтор класса: "+n.name);
  }
 }
 private void pool(ClassReader r,String where){char[] buf=new char[r.getMaxStringLength()];for(int i=1;i<r.getItemCount();i++){int at=r.getItem(i);if(at==0)continue;int tag=r.readByte(at-1);if(tag==7)type(r.readUTF8(at,buf),where);else if(tag==9||tag==10||tag==11){String owner=r.readClass(at,buf);int pair=r.getItem(r.readUnsignedShort(at+2));String name=r.readUTF8(pair,buf),desc=r.readUTF8(pair+2,buf);ref(owner,name,desc,tag==9,where);}else if(tag==16)descriptor(r.readUTF8(at,buf),where);else if(tag==17)error(where,"ConstantDynamic не поддерживается");}}
 private final class SignatureCheck extends SignatureVisitor {
  private final String where;private String owner;
  SignatureCheck(String where){super(Opcodes.ASM9);this.where=where;}
  public void visitClassType(String name){owner=name;type(name,where);}
  public void visitInnerClassType(String name){owner+="$"+name;type(owner,where);}
  public SignatureVisitor visitTypeArgument(char wildcard){return new SignatureCheck(where);}
 }
 private void signature(String sig,boolean field,String where){if(sig==null)return;SignatureReader reader=new SignatureReader(sig);if(field)reader.acceptType(new SignatureCheck(where));else reader.accept(new SignatureCheck(where));}
 private void type(String name,String where){
  if(name==null)return;if(name.startsWith("[")){descriptor(name,where);return;}
  if(name.startsWith("org/bukkit/craftbukkit/")||name.startsWith("net/minecraft/")){error(where,"NMS/CraftBukkit запрещён: "+name);return;}
  if(api(name)){if(!sdk.containsKey(name))error(where,"не поддерживается класс "+name);return;}
  if(name.startsWith("net/minecraft/")||name.startsWith("org/bukkit/craftbukkit/")){error(where,"NMS/CraftBukkit запрещён: "+name);return;}
  if(name.startsWith("ru/mcsa/")||name.startsWith("org/teavm/"))error(where,"прямой доступ к мосту запрещён: "+name);
  if(name.startsWith("java/lang/reflect/"))error(where,"рефлексия запрещена: "+name);
  if(name.equals("java/lang/Thread")||name.equals("java/lang/ThreadGroup")||(name.startsWith("java/util/concurrent/")&&!MAIN_THREAD_CONCURRENT.contains(name))||name.equals("java/util/Timer")||name.equals("java/util/TimerTask"))error(where,"собственные потоки запрещены: "+name+"; плагины исполняются на одном потоке; используйте синхронный BukkitScheduler");
  if((name.startsWith("java/io/")&&!SAFE_IO.contains(name)&&Normalize.mapped(name).equals(name))||(name.startsWith("java/nio/file/")&&Normalize.mapped(name).equals(name))||name.startsWith("java/nio/channels/")||name.startsWith("javax/net/")||name.startsWith("java/beans/")||name.equals("java/util/ServiceLoader")||name.startsWith("java/net/")||name.startsWith("sun/")||name.startsWith("jdk/")||name.equals("java/lang/Runtime")||name.equals("java/lang/ProcessBuilder")||name.equals("java/lang/ClassLoader"))error(where,"доступ к системе запрещён: "+name);
  if(!name.startsWith("java/")&&!name.startsWith("javax/")&&!name.startsWith("org/bukkit/")&&!classes.containsKey(name))error(where,"внешняя зависимость отсутствует: "+name);
 }
 private void descriptor(String desc,String where){if(desc==null)return;Type t=Type.getType(desc);if(t.getSort()==Type.METHOD){for(Type a:t.getArgumentTypes())typed(a,where);typed(t.getReturnType(),where);}else typed(t,where);}
 private void typed(Type t,String w){while(t.getSort()==Type.ARRAY)t=t.getElementType();if(t.getSort()==Type.OBJECT)type(t.getInternalName(),w);}
 private ClassNode node(String name){if(Set.of("Executor","ExecutorService","Executors","Future","Callable").stream().anyMatch(value->name.equals("java/util/concurrent/"+value))){ClassNode mapped=sdk.get("ru/mcsa/bukkit/concurrent/"+name.substring("java/util/concurrent/".length()));return mapped;}ClassNode n=sdk.containsKey(name)?sdk.get(name):classes.get(name);if(n==null&&name.startsWith("java/")){try(InputStream in=ClassLoader.getSystemResourceAsStream(name+".class")){if(in!=null){n=new ClassNode();new ClassReader(in).accept(n,ClassReader.SKIP_CODE|ClassReader.SKIP_DEBUG|ClassReader.SKIP_FRAMES);sdk.put(name,n);}}catch(IOException e){return null;}}return n;}
 private boolean bukkitParent(String owner,Set<String> seen){if(owner==null||!seen.add(owner))return false;if(api(owner))return true;ClassNode n=classes.get(owner);if(n==null)return false;if(bukkitParent(n.superName,seen))return true;for(String i:n.interfaces)if(bukkitParent(i,seen))return true;return false;}
 private boolean member(String owner,String name,String desc,boolean field,Set<String> seen){
  if(!seen.add(owner))return false;ClassNode n=node(owner);if(n==null)return false;
  if(field){for(FieldNode f:n.fields)if(f.name.equals(name)&&f.desc.equals(desc))return true;}
  else{for(MethodNode m:n.methods)if(m.name.equals(name)&&m.desc.equals(desc))return true;}
  if(name.equals("<init>"))return false;
  if(n.superName!=null&&member(n.superName,name,desc,field,seen))return true;for(String i:n.interfaces)if(member(i,name,desc,field,seen))return true;return false;
 }
 private void ref(String owner,String name,String desc,boolean field,String where){type(owner,where);descriptor(desc,where);
  if(bukkitParent(owner,new HashSet<>())&&!member(owner,name,desc,field,new HashSet<>())){
   if(owner.equals("org/bukkit/entity/Player")&&name.equals("setFlyingFallDamage"))error(where,"Правила Paper для урона при полёте не поддерживаются: setFlyingFallDamage; требуется явная настройка физики ядра");
   else error(where,"не поддерживается "+owner+"."+name+desc);
  }
  if(owner.startsWith("java/util/concurrent/")&&!Normalize.mapped(owner).equals(owner)){
   String mapped=field?new org.objectweb.asm.commons.Remapper(){public String map(String name){return Normalize.mapped(name);}}.mapDesc(desc):Normalize.mappedDescriptor(desc);
   if(!member(Normalize.mapped(owner),name,mapped,field,new HashSet<>()))error(where,"не поддерживается упорядоченный executor: "+owner+"."+name+desc);
  }
  if(owner.equals("java/util/concurrent/ConcurrentHashMap")&&(name.endsWith("Async")||name.endsWith("Parallelism")||(!field&&desc.startsWith("(J"))))error(where,"параллельные операции карты запрещены: "+owner+"."+name);
  if(!owner.startsWith("java/util/concurrent/")&&!Normalize.mapped(owner).equals(owner)){
   String mapped=field?new org.objectweb.asm.commons.Remapper(){public String map(String name){return Normalize.mapped(name);}}.mapDesc(desc):Normalize.mappedDescriptor(desc);
   if(!member(Normalize.mapped(owner),name,mapped,field,new HashSet<>()))error(where,"не поддерживается файловый API: "+owner+"."+name+desc);
  }
  if(owner.equals("java/util/concurrent/TimeUnit")&&Set.of("sleep","timedJoin","timedWait").contains(name))error(where,"ожидание потоков запрещено: TimeUnit."+name);
  if(owner.startsWith("java/lang/invoke/")&&!owner.equals("java/lang/invoke/LambdaMetafactory")&&!owner.equals("java/lang/invoke/StringConcatFactory"))error(where,"рефлексия запрещена: "+owner+"."+name);
  if(owner.equals("java/lang/Class")&&!Set.of("getName","getSimpleName","isInstance","cast","desiredAssertionStatus").contains(name))error(where,"рефлексия запрещена: Class."+name);
  if(owner.equals("java/lang/Object")&&Set.of("wait","notify","notifyAll").contains(name))error(where,"потоковый API запрещён: "+name);
  if(owner.equals("java/lang/System")&&!Set.of("arraycopy","identityHashCode","currentTimeMillis","nanoTime").contains(name))error(where,"системный API запрещён: System."+name);
 }
 private void constant(Object v,String w){if(v instanceof Type)typed((Type)v,w);else if(v instanceof Handle){Handle h=(Handle)v;ref(h.getOwner(),h.getName(),h.getDesc(),h.getTag()<=4,w);}else if(v instanceof ConstantDynamic)error(w,"ConstantDynamic не поддерживается");}
 private void annotations(List<AnnotationNode> list,String w){if(list==null)return;for(AnnotationNode a:list){descriptor(a.desc,w);if(a.values!=null)for(Object v:a.values)annotationValue(v,w);}}
 private void annotationValue(Object v,String w){if(v instanceof Type)typed((Type)v,w);else if(v instanceof AnnotationNode)annotations(List.of((AnnotationNode)v),w);else if(v instanceof List)for(Object x:(List<?>)v)annotationValue(x,w);else if(v instanceof String[])descriptor(((String[])v)[0],w);}
 private void scan(){for(ClassNode c:classes.values()){
  String w=c.name;signature(c.signature,false,w);if(c.name.equals("me/clip/placeholderapi/expansion/PlaceholderExpansion")||api(c.name)||c.name.startsWith("ru/mcsa/")||c.name.startsWith("java/")||c.name.startsWith("org/teavm/"))error(w,"подмена API запрещена");
  type(c.superName,w);for(String i:c.interfaces)type(i,w);annotations(c.visibleAnnotations,w);annotations(c.invisibleAnnotations,w);
  for(FieldNode f:c.fields){descriptor(f.desc,w);signature(f.signature,true,w);annotations(f.visibleAnnotations,w);annotations(f.invisibleAnnotations,w);constant(f.value,w);}
  for(MethodNode m:c.methods){String at=w+"."+m.name;descriptor(m.desc,at);signature(m.signature,false,at);if(m.localVariables!=null)for(LocalVariableNode local:m.localVariables){descriptor(local.desc,at);signature(local.signature,true,at);}if(m.exceptions!=null)for(String ex:m.exceptions)type(ex,at);annotations(m.visibleAnnotations,at);annotations(m.invisibleAnnotations,at);
   if((m.access&(Opcodes.ACC_NATIVE|Opcodes.ACC_SYNCHRONIZED))!=0)error(at,"native/synchronized запрещён");
   for(AbstractInsnNode i:m.instructions){if(i instanceof MethodInsnNode){MethodInsnNode n=(MethodInsnNode)i;ref(n.owner,n.name,n.desc,false,at);}else if(i instanceof FieldInsnNode){FieldInsnNode n=(FieldInsnNode)i;ref(n.owner,n.name,n.desc,true,at);}else if(i instanceof TypeInsnNode)type(((TypeInsnNode)i).desc,at);else if(i instanceof MultiANewArrayInsnNode)descriptor(((MultiANewArrayInsnNode)i).desc,at);else if(i instanceof LdcInsnNode)constant(((LdcInsnNode)i).cst,at);else if(i instanceof InvokeDynamicInsnNode){InvokeDynamicInsnNode n=(InvokeDynamicInsnNode)i;descriptor(n.desc,at);String owner=n.bsm.getOwner();if(!owner.equals("java/lang/invoke/LambdaMetafactory")&&!owner.equals("java/lang/invoke/StringConcatFactory"))error(at,"неизвестный invokedynamic: "+owner);for(Object a:n.bsmArgs)constant(a,at);}else if(i.getOpcode()==Opcodes.MONITORENTER||i.getOpcode()==Opcodes.MONITOREXIT)error(at,"мониторы/потоки запрещены");}
   if(m.tryCatchBlocks!=null)for(TryCatchBlockNode t:m.tryCatchBlocks)type(t.type,at);
   if(m.visibleAnnotations!=null)for(AnnotationNode a:m.visibleAnnotations)if(a.desc.equals("Lorg/bukkit/event/EventHandler;"))handler(c,m,a,at);
  }
 }}
 private void handler(ClassNode c,MethodNode m,AnnotationNode a,String at){
  Type[] args=Type.getArgumentTypes(m.desc);Set<String> events=Set.of("org/bukkit/event/player/PlayerSwapHandItemsEvent","org/bukkit/event/entity/EntityDamageByEntityEvent","org/bukkit/event/player/PlayerItemConsumeEvent","org/bukkit/event/player/PlayerItemHeldEvent","org/bukkit/event/player/PlayerBedEnterEvent","org/bukkit/event/player/PlayerRespawnEvent","com/destroystokyo/paper/event/player/PlayerArmorChangeEvent","io/papermc/paper/event/player/AsyncChatEvent","org/bukkit/event/player/PlayerMoveEvent","org/bukkit/event/player/PlayerInteractEvent","com/destroystokyo/paper/event/player/PlayerJumpEvent","org/bukkit/event/inventory/InventoryClickEvent","org/bukkit/event/inventory/InventoryDragEvent","org/bukkit/event/inventory/InventoryOpenEvent","org/bukkit/event/inventory/InventoryCloseEvent","org/bukkit/event/entity/EntityDamageEvent","org/bukkit/event/entity/PlayerDeathEvent","org/bukkit/event/player/PlayerJoinEvent","org/bukkit/event/player/PlayerQuitEvent","org/bukkit/event/player/AsyncPlayerChatEvent","org/bukkit/event/player/PlayerCommandPreprocessEvent","org/bukkit/event/block/BlockBreakEvent","org/bukkit/event/block/BlockPlaceEvent");
  if(args.length!=1||!events.contains(args[0].getInternalName())||!Type.getReturnType(m.desc).equals(Type.VOID_TYPE)||(m.access&Opcodes.ACC_PUBLIC)==0||(m.access&Opcodes.ACC_STATIC)!=0||(c.access&Opcodes.ACC_PUBLIC)==0){error(at,"обработчик должен быть public void с одним поддерживаемым событием в public-классе");return;}
  if(args[0].getInternalName().equals("com/destroystokyo/paper/event/player/PlayerArmorChangeEvent"))watchArmor=true;
  int priority=2;boolean ignore=false;if(a.values!=null)for(int i=0;i<a.values.size();i+=2){if(a.values.get(i).equals("priority"))priority=List.of("LOWEST","LOW","NORMAL","HIGH","HIGHEST","MONITOR").indexOf(((String[])a.values.get(i+1))[1]);if(a.values.get(i).equals("ignoreCancelled"))ignore=(Boolean)a.values.get(i+1);}
  String cn=c.name.replace('/','.').replace('$','.');String en=args[0].getClassName();handlers.add("if(priority=="+priority+" && listener instanceof "+cn+" && event instanceof "+en+(ignore?" && (!(event instanceof org.bukkit.event.Cancellable)||!((org.bukkit.event.Cancellable)event).isCancelled())":"")+") (("+cn+")listener)."+m.name+"(("+en+")event);");
 }
 private void validateCommands(Object raw){
  if(raw==null)return;if(!(raw instanceof Map)){error("plugin.yml","commands должен быть отображением");return;}
  Map<?,?> commands=(Map<?,?>)raw;Set<String> names=new HashSet<>();
  for(Object key:commands.keySet()){String name=String.valueOf(key).toLowerCase(Locale.ROOT);if(!name.matches("[a-z0-9_-]+")||CORE_COMMANDS.contains(name)||!names.add(name))error("plugin.yml","Недопустимое имя команды: "+name);}
  for(Object entry:commands.values()){
   if(entry==null)continue;if(!(entry instanceof Map)){error("plugin.yml","Описание команды должно быть отображением");continue;}
   Map<?,?> spec=(Map<?,?>)entry;for(Object key:spec.keySet())if(!List.of("description","usage","aliases","permission","permission-message").contains(key))error("plugin.yml","Не поддерживается поле команды: "+key);
   for(String field:List.of("description","usage","permission","permission-message"))if(spec.get(field)!=null&&!(spec.get(field) instanceof String))error("plugin.yml","Поле команды должно быть строкой: "+field);
   Object aliases=spec.get("aliases");if(aliases instanceof String)aliases=List.of(aliases);
   if(aliases!=null&&!(aliases instanceof List)){error("plugin.yml","aliases должен быть строкой или списком");continue;}
   if(aliases instanceof List)for(Object alias:(List<?>)aliases){String name=String.valueOf(alias).toLowerCase(Locale.ROOT);if(!(alias instanceof String)||!name.matches("[a-z0-9_-]+")||CORE_COMMANDS.contains(name)||!names.add(name))error("plugin.yml","Недопустимый псевдоним команды: "+name);}
  }
 }
 private String permissions(Object raw){
  StringBuilder source=new StringBuilder();if(raw==null)return "";if(!(raw instanceof Map)){error("plugin.yml","permissions должен быть отображением");return "";}
  if(((Map<?,?>)raw).size()>4096){error("plugin.yml","слишком много permissions");return "";}
  for(Map.Entry<?,?> entry:((Map<?,?>)raw).entrySet()){
   String name=String.valueOf(entry.getKey()).toLowerCase(Locale.ROOT);if(!name.matches("[a-z0-9_.*-]{1,256}")||!(entry.getValue() instanceof Map)){error("plugin.yml","недопустимая декларация permissions: "+name);continue;}
   Map<?,?> spec=(Map<?,?>)entry.getValue();for(Object field:spec.keySet())if(!List.of("description","default","children").contains(field))error("plugin.yml","неизвестное поле permissions: "+field);
   String value=Objects.toString(spec.get("default"),"op").toLowerCase(Locale.ROOT);if(!List.of("true","false","op","not op").contains(value)){error("plugin.yml","недопустимый default permission: "+name);continue;}
   source.append("ru.mcsa.bukkit.Permissions.declare(").append(quote(name)).append(',').append(quote(value)).append(");\n");
   Object children=spec.get("children");if(children!=null&&!(children instanceof Map)){error("plugin.yml","children permission должен быть отображением");continue;}
   if(children instanceof Map)for(Map.Entry<?,?> child:((Map<?,?>)children).entrySet()){String key=String.valueOf(child.getKey()).toLowerCase(Locale.ROOT);if(!key.matches("[a-z0-9_.*-]{1,256}")||!(child.getValue() instanceof Boolean)){error("plugin.yml","недопустимый дочерний permission");continue;}source.append("ru.mcsa.bukkit.Permissions.child(").append(quote(name)).append(',').append(quote(key)).append(',').append(child.getValue()).append(");\n");}
  }return source.toString();
 }
 @SuppressWarnings("unchecked") private void run(Path jarPath,Path out,Path sdkPath)throws Exception{
  Files.createDirectories(out);Map<String,Object> meta;
  try(JarFile api=new JarFile(sdkPath.toFile());JarFile jar=new JarFile(jarPath.toFile())){
   load(api,sdk);load(jar,classes);JarEntry entry=jar.getJarEntry("plugin.yml");if(entry==null)throw new IOException("В JAR нет plugin.yml");
   LoaderOptions options=new LoaderOptions();options.setAllowDuplicateKeys(false);options.setMaxAliasesForCollections(20);options.setCodePointLimit(1024*1024);
   Object yaml=new Yaml(new SafeConstructor(options)).load(jar.getInputStream(entry));if(!(yaml instanceof Map))throw new IOException("plugin.yml должен быть отображением");meta=(Map<String,Object>)yaml;
   for(String key:List.of("name","version","main"))if(!(meta.get(key) instanceof String)&&!(meta.get(key) instanceof Number))error("plugin.yml","нет поля "+key);
   for(String key:List.of("depend","loadbefore","libraries"))if(meta.containsKey(key))error("plugin.yml","поле "+key+" пока не поддерживается");
   validateCommands(meta.get("commands"));
   String permissionSource=permissions(meta.get("permissions"));
   Object network=meta.get("network-hosts");if(network!=null&&(!(network instanceof List)||((List<?>)network).size()>32||((List<?>)network).stream().anyMatch(value->!(value instanceof String)||!validNetworkHost((String)value))))error("plugin.yml","network-hosts должен быть списком точных DNS-имён");
   Object optional=meta.get("softdepend");if(optional!=null&&(!(optional instanceof List)||((List<?>)optional).size()>128||((List<?>)optional).stream().anyMatch(value->!(value instanceof String)||!((String)value).matches("[A-Za-z0-9_.-]{1,128}"))))error("plugin.yml","softdepend должен быть списком имён плагинов");
   placeholderAbsent=optional instanceof List&&((List<?>)optional).stream().anyMatch(value->"PlaceholderAPI".equalsIgnoreCase(String.valueOf(value)));
   if(!String.valueOf(meta.get("main")).matches("[A-Za-z_$][A-Za-z0-9_$]*(\\.[A-Za-z_$][A-Za-z0-9_$]*)*"))error("plugin.yml","недопустимый главный класс");
   if(!classes.containsKey(String.valueOf(meta.get("main")).replace('.','/')))error("plugin.yml","главный класс отсутствует");
   for(ClassNode n:classes.values()){try(InputStream in=jar.getInputStream(jar.getJarEntry(n.name+".class"))){pool(new ClassReader(in),n.name);}}
   scan();
   if(report!=null){TreeSet<String> reasons=new TreeSet<>();for(String e:errors)reasons.add(e.substring(e.indexOf(": ")+2));StringJoiner diagnostics=new StringJoiner(",\n"),missing=new StringJoiner(",\n");for(String e:errors)diagnostics.add(quote(e));for(String e:reasons)missing.add(quote(e));Files.writeString(report,"{\"diagnosticCount\":"+errors.size()+",\"missing\":[\n"+missing+"\n],\"diagnostics\":[\n"+diagnostics+"\n]}\n");}
   if(!errors.isEmpty()){for(String e:errors)System.err.println("ОТКАЗ: "+e);System.err.println("Перевод невозможен: отсутствует совместимость с перечисленными API.");System.exit(2);}
   String name=String.valueOf(meta.get("name")).toLowerCase(Locale.ROOT).replaceAll("[^a-z0-9_-]","-");if(name.isEmpty()||name.length()>64)throw new IOException("Недопустимое имя плагина");
   StringBuilder source=new StringBuilder("package mcsa.generated; public final class Main { public static void main(String[] args){ ru.mcsa.bukkit.RuntimeState.plugin=new "+meta.get("main")+"(); ru.mcsa.bukkit.RuntimeState.plugin.initialize("+quote(name)+"); ru.mcsa.bukkit.RuntimeState.dispatcher=(listener,event,priority)->{\n");
   Collections.sort(handlers);for(String h:handlers)source.append(h).append('\n');source.append("};\n").append(permissionSource);
   Object commands=meta.get("commands");if(commands!=null&&!(commands instanceof Map))throw new IOException("commands должен быть отображением");if(commands instanceof Map)for(Map.Entry<?,?> e:((Map<?,?>)commands).entrySet()){
    String command=String.valueOf(e.getKey()).toLowerCase(Locale.ROOT);if(!command.matches("[a-z0-9_-]+"))throw new IOException("Недопустимое имя команды: "+command);
    if(CORE_COMMANDS.contains(command))throw new IOException("Команда принадлежит ядру или auth: "+command);
    if(e.getValue()!=null&&!(e.getValue() instanceof Map))throw new IOException("Описание команды должно быть отображением");
    Map<?,?> spec=e.getValue() instanceof Map?(Map<?,?>)e.getValue():Collections.emptyMap();for(Object k:spec.keySet())if(!List.of("description","usage","aliases","permission","permission-message").contains(k))throw new IOException("Не поддерживается поле команды: "+k);
    source.append("ru.mcsa.bukkit.RuntimeState.plugin.declareCommand(").append(quote(command)).append(',').append(quote(Objects.toString(spec.get("description"),""))).append(',').append(quote(Objects.toString(spec.get("usage"),""))).append(",0);\n");
    if(spec.get("permission")!=null)source.append("ru.mcsa.bukkit.RuntimeState.plugin.getCommand(").append(quote(command)).append(").setPermission(").append(quote(String.valueOf(spec.get("permission")))).append(");\n");
    if(spec.get("permission-message")!=null)source.append("ru.mcsa.bukkit.RuntimeState.plugin.getCommand(").append(quote(command)).append(").setPermissionMessage(").append(quote(String.valueOf(spec.get("permission-message")))).append(");\n");
    Object aliases=spec.get("aliases");if(aliases instanceof String)aliases=List.of(aliases);if(aliases!=null&&!(aliases instanceof List))throw new IOException("aliases должен быть строкой или списком");
    if(aliases instanceof List)for(Object alias:(List<?>)aliases){String value=String.valueOf(alias).toLowerCase(Locale.ROOT);if(!value.matches("[a-z0-9_-]+")||CORE_COMMANDS.contains(value))throw new IOException("Недопустимый псевдоним команды");source.append("ru.mcsa.bukkit.RuntimeState.plugin.declareAlias(").append(quote(value)).append(',').append(quote(command)).append(',').append(quote(Objects.toString(spec.get("description"),""))).append(',').append(quote(Objects.toString(spec.get("usage"),""))).append(");\n");}

   }
   source.append("ru.mcsa.bukkit.RuntimeState.plugin.setVersion(").append(quote(String.valueOf(meta.get("version")))).append(");\n");
   Enumeration<JarEntry> resources=jar.entries();long resourceSize=0;while(resources.hasMoreElements()){JarEntry resource=resources.nextElement();String path=resource.getName();if(resource.isDirectory()||!path.matches("[A-Za-z0-9_/-][A-Za-z0-9_./-]*\\.(yml|yaml|json|txt|properties)"))continue;byte[] raw=jar.getInputStream(resource).readNBytes(4*1024*1024+1);resourceSize+=raw.length;if(raw.length>4*1024*1024||resourceSize>8*1024*1024)throw new IOException("Слишком много ресурсов JAR");String text=new String(raw,java.nio.charset.StandardCharsets.UTF_8);source.append("ru.mcsa.bukkit.RuntimeState.plugin.embedResource(").append(quote(path)).append(",new StringBuilder()");for(int i=0;i<text.length();i+=8000)source.append(".append(").append(quote(text.substring(i,Math.min(i+8000,text.length())))).append(")");source.append(".toString());\n");}
   JarEntry embedded=jar.getJarEntry("config.yml");if(embedded!=null){byte[] raw=jar.getInputStream(embedded).readNBytes(4*1024*1024+1);if(raw.length>4*1024*1024)throw new IOException("config.yml больше 4 МиБ");String configText=new String(raw,java.nio.charset.StandardCharsets.UTF_8);source.append("ru.mcsa.bukkit.RuntimeState.defaultConfig=new StringBuilder()");for(int i=0;i<configText.length();i+=8000)source.append(".append(").append(quote(configText.substring(i,Math.min(i+8000,configText.length())))).append(")");source.append(".toString();\n");}
   source.append("ru.mcsa.bukkit.RuntimeState.plugin.onLoad();ru.mcsa.bukkit.RuntimeState.plugin.onEnable();");if(watchArmor)source.append("ru.mcsa.bukkit.ArmorChanges.start();");source.append("ru.mcsa.bukkit.Host.call(24);if(args.length==123456)ru.mcsa.bukkit.Entry.keep();}}\n");
   Path java=out.resolve("src/mcsa/generated/Main.java");Files.createDirectories(java.getParent());Files.writeString(java,source);
   Files.writeString(out.resolve("plugin.toml"),"name = "+quote(name)+"\nversion = "+quote(String.valueOf(meta.get("version")))+"\napi = \"0.1\"\nmain = \"plugin.wasm\"\n");
   if(network instanceof List){StringJoiner hosts=new StringJoiner(", ","network_hosts = [","]\n");for(Object host:(List<?>)network)hosts.add(quote((String)host));Files.writeString(out.resolve("plugin.toml"),hosts.toString(),StandardOpenOption.APPEND);}
   JarEntry config=jar.getJarEntry("config.yml");if(config!=null){Path data=out.resolve("data");Files.createDirectories(data);byte[] bytes=jar.getInputStream(config).readNBytes(4*1024*1024+1);if(bytes.length>4*1024*1024)throw new IOException("config.yml больше 4 МиБ");Files.write(out.resolve("default-config.yml"),bytes);}
   System.out.println("Проверка завершена: "+name+", классов "+classes.size()+", обработчиков "+handlers.size()+". Подмножество совместимо; требуется компиляция TeaVM.");
  }
 }
 public static void main(String[] a){try{Inspect inspector=new Inspect();if(a.length>3)inspector.report=Path.of(a[3]);inspector.run(Path.of(a[0]),Path.of(a[1]),Path.of(a[2]));}catch(Exception e){System.err.println("ОТКАЗ: "+e.getMessage());System.exit(2);}}
}
