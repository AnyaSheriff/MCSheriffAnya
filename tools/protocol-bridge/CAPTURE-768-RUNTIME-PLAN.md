# План узкого исключения: native runtime exact1.21.3

Статус: НЕ РАЗРЕШЕНО. До явного решения Ани ничего не извлекать, не запускать
client/server/capture и не снимать require_runtime(). Эта страница — reviewable
план для решения, а не разрешение и не actualcapture.

Предлагаемый объём: только штатная загрузка native Linux x86_64 библиотек самим
непрозрачным Java-клиентом exact1.21.3 в отдельной GitHub/Xvfb сессии. Никакого
ручного/script unpack, чтения Java/Mojangcode/mappings, game resources, datapacks,
--reports. Клиент сам читает свои обычные ресурсы для игры; ассистент их не читает
и не выгружает. Сетевое наблюдение bounded; registry/chunk только length/SHA.

Pinned native modules по официальному metadataJSON:
- LWJGL3.3.3: lwjgl, freetype, glfw, jemalloc, openal, opengl, stb, tinyfd.
- JNA5.14.0: native dispatch; jna-platform — Java dependency, не отдельный.so.
- JTracy1.0.29: natives-linux (если native runtime запрашивается).
- Netty4.1.97.Final: native epoll linux-x86_64/unix common (если запрашивается).

Ожидаемые семейства.so по модулям: liblwjgl.so, liblwjgl_freetype.so,
libglfw.so, libjemalloc.so, libopenal.so, liblwjgl_opengl.so, liblwjgl_stb.so,
liblwjgl_tinyfd.so, JNA jnidispatch, JTracy, Netty native epoll. Это список
ожидаемых runtime библиотек, НЕ прочитанный список членов JAR: точные
внутриархивные пути/имена и фактическизагруженный subset сейчас НЕ установлены.
Утверждать «ровно эти файлы извлечены» без runtimeнаблюдения было бы неверно.

Отдельный будущий launcher должен направить LWJGL/JNA/Netty/temp в новый
изолированный ephemeral каталог; разрешать только .so по этому bounded native
набору, фиксировать имена/size/SHA без чтения кода. Любые неожиданные extractfiles
или выход изкаталога — остановка/INVALID. Если такой контроль нельзя обеспечить
штатным launcher, остановиться и сообщить ограничение, не обходить.

Готового разрешённого reuse сейчас нет: scoped exact1.21.4cache и старый
via42684071176artifacttree имеют0modern.so. Все8LWJGLnativeJAR3.3.3 SHA совпадают
1.21.3/1.21.4, но opaqueJAR не является готовой.so. JNA5.14.0 отличается; legacy
LWJGL2 не подходит. Источник: handoff runtime-reuse-20261110.json, metadata only.

Альтернатива исключению — предоставить готовый approved native runtime с
provenance/SHA и immutable CIartifact. Тогда нужен launcher без nativeJAR
classpath/fallbackextraction и bounded проверка отсутствия извлечения.

После решения: отдельно обновить acceptance до actualcapture, сначала source/
pins/negativeguards, затем native reference Login/Configuration/Play/46slots/
TPACK/movement/chunkACK/PlayerInfo и cleanup, независимый raw/SHA audit.
Core768route/publicadmission и fullsupport этим не подтверждаются.
