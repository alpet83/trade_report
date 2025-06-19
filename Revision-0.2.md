# Trade Report - Ревизия 0.2

## Обзор проекта

`trade_report` — это REST API сервер, написанный на Rust, для получения данных о торговых счетах и формирования отчётов о депозитах на криптовалютных биржах. Проект использует MySQL для хранения данных и предоставляет два основных эндпоинта: `/rtm/accounts` для списка счетов и `/rtm/deposit_report` для отчётов о депозитах. Ревизия 0.2 включает поддержку фильтрации по `account_id`, выбор колонки (`value` или `value_btc`), и временных диапазонов (`start_ts`, `end_ts`).

## Основные функции

- **Эндпоинт `/rtm/accounts`**:
  - Возвращает список торговых счетов в формате JSON.
- **Эндпоинт `/rtm/deposit_report`**:
  - Формирует отчёт о депозитах для указанного счёта.
  - Поддерживает параметры:
    - `exchange` и `account_id` или `applicant` для выбора счёта.
    - `value_column` (`value` или `value_btc`) для выбора колонки данных.
    - `period` (в часах) или `start_ts`/`end_ts` (ISO 8601) для временного диапазона.
  - Время в отчёте сериализуется в формате `YYYY-MM-DDTHH:MM:SSZ`.
- **Фильтрация по `account_id`**:
  - Все запросы к данным (`trades`, `funds_history`, `orders`) фильтруются по `account_id`.
- **Обработка ошибок SQL**:
  - Общий обработчик ошибок логирует запросы и стек вызовов.

## Структура проекта

- **`src/api/rtm.rs`**: Реализация REST API эндпоинтов `/rtm/accounts` и `/rtm/deposit_report`.
- **`src/db/`**:
  - `mysql.rs`: Подключение к MySQL.
  - `trade_data_source.rs`: Логика запросов к данным (`get_trades`, `get_funds_history`, `get_orders`).
  - `error.rs`: Обработчик ошибок SQL.
- **`src/services/deposit_basic_report.rs`**: Генерация отчётов о депозитах.
- **`src/entities/`**: Структуры данных (`TradingAccount`, `FundsHistoryRow`, `DepositBasicReport`).
- **`src/tests/deposit_report.rs`**: Тесты для отчётов.
- **`config.toml`**: Конфигурация (URL базы данных).
- **`sql/`**:
  - `configs.sql`: Таблица `config__table_map` для маппинга `applicant` на `account_id`.
  - `bitmex__funds_history.sql`: Данные о депозитах для биржи BitMEX.

## Установка и запуск

### Требования
- Rust (1.65 или новее)
- MySQL (8.0 или новее)
- Cargo

### Установка
1. Клонируйте репозиторий:
   ```bash
   git clone <repository_url>
   cd trade_report
   ```
2. Настройте `config.toml`:
   ```toml
   [mysql]
   url = "mysql://user:password@localhost:3306/trading"
   ```
3. Создайте базу данных `trading` и загрузите SQL-дампы:
   ```bash
   mysql -u user -p trading < sql/configs.sql
   mysql -u user -p trading < sql/bitmex__funds_history.sql
   ```
4. Установите зависимости:
   ```bash
   cargo build
   ```

### Запуск
```bash
cargo run
```
Сервер запускается на `http://localhost:3000`.

## API

### 1. `/rtm/accounts`
- **Метод**: GET
- **Описание**: Возвращает список торговых счетов, маппленных по `applicant`.
- **Пример запроса**:
  ```bash
  curl http://localhost:3000/rtm/accounts
  ```
- **Пример ответа**:
  ```json
  {
    "bitmex2_bot": {
      "account_id": "379832",
      "applicant": "bitmex2_bot",
      "exchange": { "name": "bitmex" },
      "monitor_enabled": true
    }
  }
  ```

### 2. `/rtm/deposit_report`
- **Метод**: GET
- **Описание**: Формирует отчёт о депозитах для указанного счёта.
- **Параметры**:
  - `exchange` (опционально): Название биржи (например, `bitmex`).
  - `account_id` (опционально): ID счёта (например, `379832`).
  - `applicant` (опционально): Идентификатор бота (например, `bitmex2_bot`).
  - `value_column` (опционально): Колонка данных (`value` или `value_btc`, по умолчанию `value`).
  - `period` (опционально): Период в часах (по умолчанию `24`).
  - `start_ts` (опционально): Начало периода (ISO 8601, например, `2025-06-18T14:00:00Z`).
  - `end_ts` (опционально): Конец периода (ISO 8601, например, `2025-06-18T15:00:00Z`).
- **Логика времени**:
  - Если `end_ts` не задан, используется текущее время (`Utc::now()`).
  - Если `start_ts` не задан, вычисляется как `end_ts - period` часов, округлённое до начала часа.
- **Пример запроса**:
  ```bash
  curl "http://localhost:3000/rtm/deposit_report?applicant=bitmex2_bot&value_column=value_btc&start_ts=2025-06-18T14:00:00Z&end_ts=2025-06-18T15:00:00Z"
  ```
- **Пример ответа**:
  ```json
  {
    "account_id": "379832",
    "exchange": "bitmex",
    "start_ts": "2025-06-18T14:00:00Z",
    "end_ts": "2025-06-18T15:00:00Z",
    "start_value": 0.951331,
    "end_value": 0.950969,
    "change_percent": -0.0381,
    "value_column": "value_btc"
  }
  ```

## Использование

### Примеры запросов
1. Получить отчёт за последние 48 часов:
   ```bash
   curl "http://localhost:3000/rtm/deposit_report?applicant=bitmex2_bot&period=48&value_column=value"
   ```
2. Получить отчёт за конкретный период:
   ```bash
   curl "http://localhost:3000/rtm/deposit_report?exchange=bitmex&account_id=379832&start_ts=2025-06-18T14:00:00Z&end_ts=2025-06-18T15:00:00Z&value_column=value_btc"
   ```
3. Получить список счетов:
   ```bash
   curl http://localhost:3000/rtm/accounts
   ```

### Логи
Логи сервера выводятся в консоль с уровнями `INFO`, `DEBUG`, и `ERROR`. Пример лога для `/rtm/deposit_report`:
```
[2025-06-19T11:59:00Z INFO] Starting deposit report request
[2025-06-19T11:59:00Z DEBUG] Selected account_id: 379832, exchange: bitmex
[2025-06-19T11:59:00Z DEBUG] Using time range: start_ts=2025-06-18T14:00:00Z, end_ts=2025-06-18T15:00:00Z
[2025-06-19T11:59:00Z INFO] Deposit report request completed
```

## Ограничения
- Методы `get_candle_price`, `get_ticker_info`, `get_deposit_history`, `get_position_history`, `get_trade_signals`, и `get_report_configs` в `trade_data_source.rs` не реализованы.
- Поддерживается только биржа BitMEX (таблицы `bitmex__funds_history`, `bitmex__trades`, `bitmex__orders`).
- Ошибки SQL логируются, но параметры запроса не включаются в лог.

## Метаданные
- **Ревизия**: 0.2
- **Дата создания документации**: 2025-06-19 12:13:00 EEST
- **Артефакты**:
  - `rtm.rs`: `artifact_id="8175e37b-d181-43f5-b114-c752a846470a"`, `version_id="511de385-54df-4657-9c48-3f9c1e311d52"`
  - `deposit_basic_report.rs`: `artifact_id="36a66a8d-2905-43fc-afd4-e1467c56b1e8"`, `version_id="afed9471-afb6-4992-8c22-6b63a34a0b30"`
  - `trade_data_source.rs`: `artifact_id="16c678c2-0a49-4e86-97b1-228913ed9431"`, `version_id="852c7d38-41ea-40d6-8f06-9f00f87ffb18"`
  - `error.rs`: `artifact_id="9723c877-e3db-4a13-b5fb-415d2508e70f"`, `version_id="f86171f1-0724-4b25-a769-e83473f57440"`
  - `deposit_report.rs`: `artifact_id="26837d4d-3726-4c1e-ac04-b900197934b7"`, `version_id="c8654461-229a-4d39-8093-602786ff8493"`
  - `mod.rs`: `artifact_id="b7490315-e695-436e-8544-395f05fd1fad"`, `version_id="5aec2ba2-6073-4996-8c10-ea09ebb44518"`