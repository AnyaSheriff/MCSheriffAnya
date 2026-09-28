# Приёмник отчётов за nginx — для сервера, где 80 и 443 делятся с xray
# через stream по SNI (как на tg.yebat.su). Кладётся в
# /etc/nginx/sites-available/ и включается ссылкой из sites-enabled/.
#
# Отчёты принимаются на своём порту 7443: сюда nginx слушает сам, мимо
# stream, и видит настоящий адрес отправителя — приёмник ограничивает
# каждый адрес. Через 443 (stream) адрес теряется, поэтому там только
# /health: иначе через него можно было бы обойти предел.
#
# Журнала запросов нет (access_log off): адреса отправителей не пишутся.

# ACME-проверка и переход на https (сюда stream отдаёт обычный HTTP с 80).
server {
    listen 127.0.0.1:8081;
    server_name log.sheriffanya.ru;
    root /var/www/html;
    access_log off;

    location /.well-known/acme-challenge/ { }
    location / { return 301 https://$host:7443$request_uri; }
}

# Через 443 (stream по SNI): только «жив ли».
server {
    listen 127.0.0.1:8443 ssl;
    server_name log.sheriffanya.ru;
    access_log off;

    ssl_certificate     /etc/letsencrypt/live/log.sheriffanya.ru/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/log.sheriffanya.ru/privkey.pem;

    location = /health { proxy_pass http://127.0.0.1:8787; }
    location / { return 404; }
}

# Сам приёмник — на своём порту, с настоящим адресом отправителя.
server {
    listen 7443 ssl;
    server_name log.sheriffanya.ru;
    access_log off;

    ssl_certificate     /etc/letsencrypt/live/log.sheriffanya.ru/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/log.sheriffanya.ru/privkey.pem;

    client_max_body_size 256k;
    client_body_timeout 10s;

    location / {
        proxy_pass http://127.0.0.1:8787;
        proxy_set_header Host $host;
        # Адрес отправителя нужен приёмнику только в памяти — для предела
        # частоты; nginx его затирает, а не дописывает к присланному.
        proxy_set_header X-Forwarded-For $remote_addr;
    }
}
