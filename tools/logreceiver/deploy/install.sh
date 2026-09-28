#!/bin/sh
# Установка приёмника на Debian 13. Запускать от root в папке, где лежат
# mcsa-logreceiver и файлы из deploy/:
#
#   sh install.sh          — только приёмник (веб-сервер уже есть: nginx,
#                            настройка — nginx-log.sheriffanya.ru, см. README);
#   WEB=caddy sh install.sh — ещё и Caddy с HTTPS: для пустого сервера, где
#                            порты 80 и 443 свободны.
#
# Повторный запуск обновляет программу и настройки; ключ и отчёты остаются.
set -eu

cd "$(dirname "$0")"

install -m 0755 mcsa-logreceiver /usr/local/bin/mcsa-logreceiver
install -m 0644 mcsa-logreceiver.service /etc/systemd/system/mcsa-logreceiver.service

# Ключ для чтения отчётов — только при первой установке.
install -d -m 0700 /etc/mcsa-logreceiver
if [ ! -s /etc/mcsa-logreceiver/token ]; then
	head -c 32 /dev/urandom | base64 | tr -d '=+/\n' > /etc/mcsa-logreceiver/token
	chmod 0600 /etc/mcsa-logreceiver/token
	echo "Создан ключ для чтения: /etc/mcsa-logreceiver/token"
fi

systemctl daemon-reload
systemctl enable mcsa-logreceiver
systemctl restart mcsa-logreceiver

if [ "${WEB:-}" = "caddy" ]; then
	apt-get update
	apt-get install -y caddy

	# Настройку Caddy заменяем, только если там заглушка из пакета или уже
	# наш сайт; прежняя остаётся рядом копией. Чужие сайты не трогаем.
	caddyfile=/etc/caddy/Caddyfile
	if [ ! -s "$caddyfile" ] || grep -q "log.sheriffanya.ru" "$caddyfile" \
		|| { grep -q "/usr/share/caddy" "$caddyfile" && ! grep -q "reverse_proxy" "$caddyfile"; }; then
		[ -s "$caddyfile" ] && cp "$caddyfile" "$caddyfile.bak.$(date +%Y%m%d%H%M%S)"
		install -m 0644 Caddyfile "$caddyfile"
	else
		echo "В $caddyfile уже есть другие сайты — допишите туда блок из Caddyfile сами"
	fi
	systemctl reload caddy || systemctl restart caddy
fi

sleep 1
if [ "$(wget -qO- http://127.0.0.1:8787/health 2>/dev/null || curl -fsS http://127.0.0.1:8787/health 2>/dev/null)" = "ok" ]; then
	echo "Приёмник работает на 127.0.0.1:8787"
else
	echo "Приёмник не отвечает — смотрите: journalctl -u mcsa-logreceiver"
	exit 1
fi
