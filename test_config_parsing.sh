#!/bin/bash

echo "=== Тест парсинга конфигурации серверов ==="

# Тестовые данные (имитируем HOSTS_LIST)
HOSTS_CONF_ENV='139.162.150.114
  default_mode = "server"
  server_tcp_port = 8080
  server_workers = "50"
  server_tls_port = 443
  cert_path = "/root/specure-cd.crt"
  key_path = "/root/specure-cd.key"
  logger = "info"
  server_registration = true
  registration_token = "$SECRET_TOKEN"
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
  registration_token = "$SECRET_TOKEN"
  hostname = "dev.measurementservers.net"
  server_name = "Rust Auto Registered"
  x_nettest_client = "nt"
  control_server = "https://api.nettest.org"'

echo "Тестовые данные загружены"
echo "Длина: ${#HOSTS_CONF_ENV} символов"
echo "---"

# Тест 1: Извлечение IP адресов
echo "Тест 1: Извлечение IP адресов"
IP_ADDRESSES=$(echo "$HOSTS_CONF_ENV" | grep -E "^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+" | tr '\n' ' ' | sed 's/ $//')

echo "Найденные IP: '$IP_ADDRESSES'"
echo "Количество IP: $(echo "$IP_ADDRESSES" | wc -w)"

# Проверяем результат
EXPECTED_IPS="139.162.150.114 10.35.2.151"
if [ "$IP_ADDRESSES" = "$EXPECTED_IPS" ]; then
    echo "✅ Тест 1 пройден: IP адреса найдены корректно"
else
    echo "❌ Тест 1 провален!"
    echo "Ожидалось: '$EXPECTED_IPS'"
    echo "Получено: '$IP_ADDRESSES'"
fi
echo "---"

# Тест 2: Извлечение конфигурации для конкретного IP
echo "Тест 2: Извлечение конфигурации для 139.162.150.114"

CONFIG_139=$(echo "$HOSTS_CONF_ENV" | awk -v ip="139.162.150.114" '
BEGIN { in_config = 0; }
/^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+/ {
    if ($0 ~ "^" ip) {
        in_config = 1;
        next;
    } else {
        in_config = 0;
    }
}
/^---$/ {
    in_config = 0;
    next;
}
in_config && /^[[:space:]]*[a-zA-Z_][a-zA-Z0-9_]*[[:space:]]*=/ {
    gsub(/^[[:space:]]*/, "");
    print;
}
')

echo "Конфигурация для 139.162.150.114:"
echo "$CONFIG_139"
echo "---"

# Тест 3: Извлечение конфигурации для второго IP
echo "Тест 3: Извлечение конфигурации для 10.35.2.151"

CONFIG_10=$(echo "$HOSTS_CONF_ENV" | awk -v ip="10.35.2.151" '
BEGIN { in_config = 0; }
/^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+/ {
    if ($0 ~ "^" ip) {
        in_config = 1;
        next;
    } else {
        in_config = 0;
    }
}
/^---$/ {
    in_config = 0;
    next;
}
in_config && /^[[:space:]]*[a-zA-Z_][a-zA-Z0-9_]*[[:space:]]*=/ {
    gsub(/^[[:space:]]*/, "");
    print;
}
')

echo "Конфигурация для 10.35.2.151:"
echo "$CONFIG_10"
echo "---"

# Тест 4: Подстановка SECRET_TOKEN
echo "Тест 4: Подстановка SECRET_TOKEN"
SECRET_TOKEN="test_secret_123"

CONFIG_WITH_TOKEN=$(echo "$CONFIG_139" | sed 's/\$SECRET_TOKEN/'"$SECRET_TOKEN"'/g')

echo "Конфигурация с подставленным токеном:"
echo "$CONFIG_WITH_TOKEN"
echo "---"

# Проверяем что токен заменился
if echo "$CONFIG_WITH_TOKEN" | grep -q "$SECRET_TOKEN"; then
    echo "✅ Тест 4 пройден: SECRET_TOKEN успешно подставлен"
else
    echo "❌ Тест 4 провален: SECRET_TOKEN не заменился"
fi

echo "=== Все тесты завершены ==="
