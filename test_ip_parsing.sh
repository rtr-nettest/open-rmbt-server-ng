#!/bin/bash

# Тест для парсинга IP адресов из конфигурации хостов

echo "=== Тест парсинга IP адресов ==="

# Тестовые данные (имитируем HOSTS_CONF_ENV)
HOSTS_CONF_ENV='139.162.150.114
  default_mode = "server"
  server_tcp_port = 8080
  server_workers = "50"
  server_tls_port = 443
  cert_path = "/root/specure-cd.crt"
  key_path = "/root/specure-cd.key"
  logger = "info"
  server_registration = true
  registration_token = "specure_secret_token"
  hostname = "frankfurt2025.measurementservers.net"
  server_name = "Frankfurt 2025 Rust Server Autoregistered"
  x_nettest_client = "nt"
  control_server = "https://api.nettest.org"

---

10.35.2.151
  default_mode = "server"
  server_tcp_port = 8080
  server_workers = "50"
  server_tls_port = 443
  cert_path = "/root/specure-cd.crt"
  key_path = "/root/specure-cd.key"
  logger = "info"
  server_registration = true
  registration_token = "specure_secret_token"
  hostname = "dev.measurementservers.net"
  server_name = "Rust Auto Registered"
  x_nettest_client = "nt"
  control_server = "https://api.nettest.org"'

echo "Тестовые данные:"
echo "$HOSTS_CONF_ENV"
echo "---"

# Тестируем наш алгоритм
echo "Запускаем парсинг..."
IP_ADDRESSES=""
while IFS= read -r block; do
    # Убираем пустые строки и берем первую строку
    FIRST_LINE=$(echo "$block" | grep -v '^[[:space:]]*$' | head -1)
    echo "Блок: '$block'"
    echo "Первая строка: '$FIRST_LINE'"
    
    if [[ $FIRST_LINE =~ ^[[:space:]]*([0-9]+\.[0-9]+\.[0-9]+\.[0-9]+)$ ]]; then
        echo "✅ Найден IP: ${BASH_REMATCH[1]}"
        IP_ADDRESSES="$IP_ADDRESSES ${BASH_REMATCH[1]}"
    else
        echo "❌ Не IP адрес: '$FIRST_LINE'"
    fi
    echo "---"
done < <(echo "$HOSTS_CONF_ENV" | awk 'BEGIN{RS="---"} {print $0}')

IP_ADDRESSES="${IP_ADDRESSES# }"  # убираем первый пробел

echo "Результат:"
echo "IP_ADDRESSES='$IP_ADDRESSES'"
echo "Количество IP: $(echo "$IP_ADDRESSES" | wc -w)"

# Проверяем результат
EXPECTED_IPS="139.162.150.114 10.35.2.151"
if [ "$IP_ADDRESSES" = "$EXPECTED_IPS" ]; then
    echo "✅ Тест пройден! Найдены все ожидаемые IP адреса"
else
    echo "❌ Тест провален!"
    echo "Ожидалось: '$EXPECTED_IPS'"
    echo "Получено: '$IP_ADDRESSES'"
    exit 1
fi

echo "=== Тест завершен ==="
