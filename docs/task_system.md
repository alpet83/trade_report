# Task System Documentation

## Overview

The task system in the `trade_report` project (v0.3.0) enables background, sequential execution of computational tasks without blocking the main application thread. This ensures that long-running or resource-intensive operations, such as data aggregation or cache loading, do not interfere with the responsiveness of the main API server. The system is built around two core components:

- **Task Trait**: Defines the interface for tasks, specifying methods for initialization, execution, and cleanup.
- **TaskProcessor**: Manages a queue of tasks, executing them sequentially in a dedicated background thread and handling task scheduling, completion, and cleanup.

This design supports non-blocking operations, making it suitable for real-time API endpoints (e.g., `/trades_aggregated` in `rtm.rs`) and background data processing (e.g., trade aggregation, price cache loading).

## Task Trait

The `Task` trait, defined in `src/entities/task.rs`, provides a standardized interface for tasks that can be executed by the `TaskProcessor`. It ensures that tasks are initialized, executed, and released in a controlled manner, with support for status tracking and result storage.

### Methods
- **init**: Initializes the task, preparing any necessary resources (e.g., database connections, caches). Returns `Result<(), String>` to indicate success or failure.
- **run**: Executes the task's core logic, returning a `TaskStatus` (`New`, `Postponed`, `Scheduled`, `Completed`, or `Failed`). Errors are handled gracefully to avoid crashing the `TaskProcessor`.
- **release**: Cleans up resources (e.g., clears memory, closes connections) after execution. Returns `Result<(), String>`.
- **status**: Returns the current `TaskStatus` of the task.
- **set_status**: Updates the task's status.
- **result**: Returns the task's result as a `serde_json::Value` (e.g., a string or JSON object summarizing the outcome).
- **set_result**: Sets the task's result.
- **start_at**: Returns the scheduled start time (`DateTime<Utc>`).
- **set_start_at**: Sets the scheduled start time.
- **id**: Returns the unique task ID (`u32`).
- **set_id**: Sets the task ID.

### TaskBase
The `TaskBase` struct provides common fields (`id`, `status`, `result`, `start_at`) and default implementations for status, result, and scheduling methods. Tasks implement the `Task` trait and typically embed `TaskBase` to reuse this functionality.

## TaskProcessor

The `TaskProcessor`, defined in `src/services/task_processor.rs`, is a singleton that manages a queue of tasks and runs them in a background thread. It ensures sequential execution, preventing resource contention, and supports task scheduling, rescheduling, and cleanup.

### Key Features
- **Background Thread**: Runs tasks in a dedicated Tokio thread, ensuring the main thread (e.g., Axum server) remains unblocked.
- **Task Queues**: Maintains two `DashMap` collections:
  - `scheduled`: Tasks waiting to be executed, keyed by their start time (`DateTime<Utc>`).
  - `completed`: Successfully completed tasks, retained for 10 minutes before cleanup.
- **Task Lifecycle**:
  - Tasks are added with `add`, which initializes and schedules them.
  - Tasks are executed when their `start_at` time is reached.
  - Completed tasks are moved to the `completed` queue; failed tasks are released.
  - Tasks can be rescheduled (`replay_at`) or removed (`remove`).
- **Error Handling**: Failed tasks increment a `failed_count` counter and are released to free resources.
- **Status Monitoring**: The `print_status` method logs the number of scheduled, completed, and failed tasks.

### Usage
The `TaskProcessor` is initialized in `main.rs` during application startup. Tasks are added via `TaskProcessor::add` and can be registered automatically during task creation (e.g., with `auto_reg=true` in task constructors).

## Implemented Task Implementations

Two tasks currently implement the `Task` trait, supporting specific use cases in the `trade_report` project.

### 1. TradesAggregator
- **File**: `src/entities/trades_aggregator.rs`
- **Purpose**: Aggregates trades into virtual trades based on time windows (`Coarse` mode) or direction changes (`Precise` mode).
- **Functionality**:
  - **Coarse Mode**: Groups trades into fixed time intervals (e.g., 1d, 7d, 30d) with optional week alignment (Monday-to-Sunday windows). Aggregates buys and sells separately within each window.
  - **Precise Mode**: Groups trades by direction (buy/sell), closing a window when the direction changes.
  - Stores results in `results: Vec<Trade>`, accessible after execution.
- **Key Methods**:
  - `new`: Creates a new aggregator with a `TradesCache`, time range, interval, and aggregation method (`CalcMethod::Coarse` or `Precise`).
  - `aggregate_coarse`: Performs coarse aggregation, producing virtual trades for each time window.
  - `aggregate_precise`: Performs precise aggregation, producing virtual trades based on direction changes.
  - `run`: Executes aggregation based on `calc_method` (used only by `TaskProcessor`).
- **Usage**: Integrated into the `/trades_aggregated` endpoint in `rtm.rs`. Can be run directly (`aggregate_coarse`/`aggregate_precise`) or via `TaskProcessor` with `auto_reg=true`.
- **Example**:
  - Aggregates trades for `account_id=1`, `pair_id=1` (BTC) from 2024-12-01 to 2025-01-28 with `coarse_interval=7d` or `precise_comb=1`.

### 2. LoadPriceCacheTask
- **File**: `src/services/cache/price.rs`
- **Purpose**: Prefetches VWAP (Volume-Weighted Average Price) data for a given exchange and pair, storing it in a `PriceCache`.
- **Functionality**:
  - Loads candlestick data (`Candle`) from the database for a specified time range (with a one-day buffer).
  - Calculates hourly VWAP prices and stores them in `PriceCache::data` (a `DashMap<i32, f32>`).
  - Used to provide price data for equity calculations and trade aggregation.
- **Key Methods**:
  - `new`: Creates a new task with a `PriceCache`, time range, and optional auto-registration.
  - `run`: Executes `PriceCache::load_prefetch` to load VWAP data.
- **Usage**: Automatically scheduled during `Exchange` initialization (`exchange.rs`) for BTC (`pair_id=1`) over the past year. Can be triggered manually via `PriceCache::load_prefetch`.
- **Example**:
  - Prefetches VWAP prices for `bitmex` exchange, `pair_id=1`, from 2024-06-25 to 2025-06-25.

## Usage in the Project
- **Initialization**: `TaskProcessor::init` is called in `main.rs` to start the background thread.
- **Task Registration**:
  - Tasks like `TradesAggregator` and `LoadPriceCacheTask` can be registered with `auto_reg=true` during creation, adding them to the `TaskProcessor` queue.
  - Alternatively, tasks can be executed directly (e.g., `TradesAggregator::aggregate_coarse`) in API endpoints for immediate results.
- **Monitoring**: Use `TaskProcessor::print_status` to log the status of scheduled and completed tasks.
- **Error Handling**: Tasks return `TaskStatus::Failed` on errors, with details stored in `result`. The `TaskProcessor` tracks failed tasks via `failed_count`.

## Notes
- **Thread Safety**: Both `TaskProcessor` and task implementations use `Arc` and `RwLock` for thread-safe access to shared resources (e.g., `TradesCache`, `PriceCache`).
- **Performance**: Sequential execution ensures predictable resource usage but may limit throughput for high-frequency tasks. Future improvements could include parallel task queues.
- **Testing**: Tests for `TaskProcessor` are in `tests/task.rs` (`test_task_processor_add_and_run`), and for `TradesAggregator` in `tests/trades_aggregator.rs` (`test_trades_aggregated_endpoint`).

## Future Improvements
- Add support for task prioritization or parallel execution for non-conflicting tasks.
- Implement persistent task storage to recover tasks after application restarts.
- Enhance `TaskProcessor::print_status` with detailed task metadata (e.g., task type, parameters).