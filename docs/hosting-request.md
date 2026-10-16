Здравствуйте, команда hosting-minecraft.pro!

Прошу рассмотреть добавление MCSheriffAnya в список серверных ядер — хотя бы
для моего сервера, если пока рано включать его в общий каталог.

Это самостоятельное ядро на Rust с открытым исходным кодом под лицензией
MIT. Проект находится в альфа-стадии: поддерживает Minecraft Java 26.1.2
(протокол 775) и частично Bedrock 26.1.10–26.1.13 (протокол 944).
Java Runtime не требуется.

- GitHub: https://github.com/AnyaSheriff/MCSheriffAnya
- Docker-образ: `sheriffanya/mcsa`; для Pterodactyl / Pelican подготовлен
  вариант `sheriffanya/mcsa:panel` (версионный тег: `<version>-panel`).
- Egg: https://github.com/AnyaSheriff/MCSheriffAnya/blob/main/docker/pterodactyl/egg-mcsheriffanya.json
- Инструкция: https://github.com/AnyaSheriff/MCSheriffAnya/blob/main/docs/hosting.md
- Порты по умолчанию: TCP 25565 для Java и UDP 19132 для Bedrock; можно
  использовать выделенные вами номера, а Bedrock при необходимости отключить.

Бинарник входит в образ, конфигурация и мир хранятся в каталоге сервера,
остановка — командой `stop`. Интеграция требует проверки на вашей версии
панели и наличия опубликованного тега образа. Буду благодарна за возможность
тестового размещения и информацию о подходящем тарифе и дальнейших шагах.

Спасибо!
