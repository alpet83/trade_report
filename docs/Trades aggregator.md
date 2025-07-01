Trades Aggregator Algorithm Specification
This document formalizes the algorithms for aggregating trades in the trade_report project (v0.3.0), as implemented in src/entities/trades_aggregator.rs. The specification covers two aggregation methods: Coarse and Precise, used to generate virtual trades from raw trade data for statistical reporting. The algorithms are designed to be thread-safe, asynchronous, and integrated with TaskProcessor in a Rust-based server application.
1. Overview
The TradesAggregator processes a list of Trade structs from TradesCache, grouping them into virtual trades based on time windows (Coarse) or direction changes (Precise). Virtual trades summarize buy or sell activities, computing net volumes and weighted average prices. The implementation uses chrono for timestamp handling, serde_json for output serialization, dashmap for thread-safe storage, and auto_round for price/amount precision.
Key Components

Trade Struct: Defined in src/entities/trade.rs:struct Trade {
    ts: DateTime<Utc>,    // Timestamp (e.g., "2024-01-01T00:00:00Z")
    pair_id: i32,         // Pair identifier (e.g., BTC_PAIR_ID=1)
    buy: bool,            // true for buy, false for sell
    price: f32,           // Trade price
    amount: f32,          // Trade volume
    trade_no: String,     // Unique trade identifier
    order_id: u32,        // Order identifier (set to 0 in virtual trades)
    position: f32,        // Position size (set to 0)
    rpnl: f32,            // Realized P&L (set to 0)
    flags: i32,           // Number of source trades in virtual trade
    comission: f32,       // Commission (set to 0)
}


Input Data: Loaded from sample_trades_extended.csv via TradesCache::import_csv.
Output: JSON array of virtual trades, serialized via serde_json.
Interval Functions: Defined in src/common/interval_func.rs for timestamp alignment.

2. Coarse Aggregation Algorithm
The Coarse algorithm groups trades within fixed time windows and produces up to two virtual trades per window (one buy, one sell), summarizing net volume and average price.
Inputs

Trades: List of Trade structs from TradesCache.get_trades(start_ts, end_ts).
start_ts: Start of the aggregation period (e.g., 2024-01-01T00:00:00Z).
end_ts: End of the aggregation period (e.g., 2025-03-31T00:00:00Z).
interval: Time window duration (chrono::Duration), e.g.:
1h: 3600 seconds
1d: 86,400 seconds
7d: 604,800 seconds
30d: 2,592,000 seconds
90d: 7,776,000 seconds
365d: 31,536,000 seconds


week_align: Boolean flag:
true for intervals ≥7 days: Align windows to Monday 00:00:00 UTC.
false for intervals ≥30 days: Align to the 1st of the month.
Ignored for intervals <7 days.



Algorithm Steps

Retrieve and Sort Trades:

Call TradesCache.get_trades(start_ts, end_ts) to fetch trades.
Sort trades by ts (ascending) to ensure chronological processing.
Example: For sample_trades_extended.csv with start_ts=2024-01-01T00:00:00Z, end_ts=2025-03-31T00:00:00Z, trades include:
buy_q1_1 (2024-01-01 00:00:00, 100@40000)
sell_q1_1 (2024-01-01 01:00:00, 90@40500)
...
buy_q1_25_1 (2025-03-01 00:00:00, 190@65000)
sell_q1_25_1 (2025-03-01 01:00:00, 180@65500)




Determine Time Windows:

For intervals <7 days (e.g., 1h, 1d):
Windows start at start_ts + n * interval, where n is an integer.
Example: For 1h, start_ts=2024-01-01T00:00:00Z:
Window 1: 2024-01-01T00:00:00Z to 2024-01-01T00:59:59.999Z
Window 2: 2024-01-01T01:00:00Z to 2024-01-01T01:59:59.999Z




For intervals ≥7 days with week_align=true (e.g., 7d, 30d_weekly, 90d, 365d):
Use adjust_to_monday to align to the Monday of the week containing the trade’s ts.
Example: For 2024-12-31T23:00:00Z (Tuesday), adjust_to_monday returns 2024-12-30T00:00:00Z (Monday of week 53, 2024).
Window: Monday 00:00:00 to Sunday 23:59:59.999.


For intervals ≥30 days with week_align=false (e.g., 30d):
Use adjust_to_first_of_month to align to the 1st of the month.
Example: For 2024-12-31T23:00:00Z, adjust_to_first_of_month returns 2025-01-01T00:00:00Z if ts.month() == 12 and ts.day() >= 25, else 2024-12-01T00:00:00Z.


For 90d, week_align=true:
Use adjust_to_first_of_quarter to align to the 1st of the quarter (e.g., 1 Jan, 1 Apr, 1 Jul, 1 Oct), then adjust_to_monday.
Example: For 2024-10-01T00:00:00Z (Tuesday), adjust_to_first_of_quarter → 2024-10-01T00:00:00Z, then adjust_to_monday → 2024-09-30T00:00:00Z.


For 365d, week_align=true:
Use adjust_to_first_of_year to align to 1 Jan, then adjust_to_monday.
Example: For 2025-01-01T00:00:00Z (Wednesday), adjust_to_first_of_year → 2025-01-01T00:00:00Z, then adjust_to_monday → 2024-12-30T00:00:00Z.




Group Trades by Window and Direction:

Create two HashMap<DateTime<Utc>, Vec<&Trade>> for buys and sells.
For each trade, compute its window start using the appropriate adjust_* function.
Add trade to buys or sells based on trade.buy.
Example: For 7d, week_align=true:
buy_year_1 (2024-12-31 23:00:00, 150@60000) → 2024-12-30T00:00:00Z (buy).
sell_year_1 (2024-12-31 23:15:00, 140@60500) → 2024-12-30T00:00:00Z (sell).




Aggregate Trades per Window:

For each unique window start time:
For buys: Compute:
total_amount = Σ(amount) (e.g., 150 + 160 = 310 for buy_year_1:buy_year_2).
total_price_volume = Σ(price * amount) (e.g., 150*60000 + 160*61000 = 18,760,000).
avg_price = total_price_volume / total_amount (e.g., 18,760,000 / 310 ≈ 60516.129).
Apply auto_round(avg_price, 0) → 60516.1.
Apply auto_round(total_amount, 0) → 310.
trade_no = "buy_year_1:buy_year_2".
Create virtual trade: {ts: "2024-12-30T00:00:00Z", buy: true, price: 60516.1, amount: 310, trade_no: "buy_year_1:buy_year_2", flags: 2, ...}.


For sells: Similar process.
Skip empty groups (no virtual trade if no buys/sells in window).


Example: For 90d, window 2024-09-30T00:00:00Z:
Buys: buy_q4_1 (140@55000), buy_year_1 (150@60000), buy_year_2 (160@61000).
total_amount = 140 + 150 + 160 = 450.
total_price_volume = 140*55000 + 150*60000 + 160*61000 = 26,460,000.
avg_price = 26,460,000 / 450 ≈ 58,800.
Virtual trade: {ts: "2024-09-30T00:00:00Z", buy: true, price: 58800, amount: 450, trade_no: "buy_q4_1:buy_year_2", flags: 3, ...}.




Sort and Output:

Sort virtual trades by ts (ascending), with buys before sells for equal ts.
Serialize to JSON array using serde_json::to_value.
Example output for 7d:[
  {"ts": "2024-01-01T00:00:00Z", "buy": true, "price": 40000, "amount": 100, "trade_no": "buy_q1_1", "flags": 1, ...},
  {"ts": "2024-01-01T00:00:00Z", "buy": false, "price": 40500, "amount": 90, "trade_no": "sell_q1_1", "flags": 1, ...},
  ...
  {"ts": "2024-12-30T00:00:00Z", "buy": true, "price": 61575.8, "amount": 660, "trade_no": "buy_year_1:buy_year_4", "flags": 4, ...},
  {"ts": "2024-12-30T00:00:00Z", "buy": false, "price": 62080.6, "amount": 620, "trade_no": "sell_year_1:sell_year_4", "flags": 4, ...}
]





Notes

Window Boundaries: Windows are closed (e.g., [start, start + interval)), ensuring no overlap.
Rounding: auto_round (from src/common/math.rs) ensures consistent precision:
For price ≥10000: 1 decimal place (e.g., 55073.767 → 55073.8).
For price ≥1000: 2 decimal places (e.g., 4285.030 → 4285.03).
For price <1: 5 decimal places.
amount rounded to 0 decimal places.


Thread-Safety: Uses Arc<TradesCache> and DashMap for concurrent access.
Logging: Follows EDA Rule 4 with #DBG for trade aggregation details and #INFO for completion.

3. Precise Aggregation Algorithm
The Precise algorithm groups trades by direction changes (buy to sell or sell to buy), producing virtual trades that summarize consecutive same-direction trades.
Inputs

Trades, start_ts, end_ts: Same as Coarse.
interval, week_align: Ignored (no time-based windowing).

Algorithm Steps

Retrieve and Sort Trades:

Fetch and sort trades by ts (ascending), as in Coarse.
Example: Same trades from sample_trades_extended.csv.


Group Trades by Direction:

Initialize current_trades: Vec<&Trade> (empty) and current_direction: Option<bool> (None).
For each trade:
If current_direction is None, set it to trade.buy and add trade to current_trades.
If trade.buy == current_direction, add trade to current_trades.
If trade.buy != current_direction:
Aggregate current_trades into a virtual trade (see below).
Clear current_trades, add current trade, set current_direction = trade.buy.




After loop, aggregate any remaining current_trades.
Example: Trades [buy_q1_1, sell_q1_1, buy_q2_1, sell_q2_1]:
Group 1: [buy_q1_1] (buy).
Group 2: [sell_q1_1] (sell).
Group 3: [buy_q2_1] (buy).
Group 4: [sell_q2_1] (sell).




Aggregate Trades per Group:

For each group:
total_amount = Σ(amount).
total_price_volume = Σ(price * amount).
avg_price = total_price_volume / total_amount (or 0.0 if total_amount=0).
Round avg_price and total_amount using auto_round.
trade_no = "<first_trade_no>:<last_trade_no>" or single trade_no.
Create virtual trade with ts of the first trade in the group.
Example: For buy_year_1 (150@60000), buy_year_2 (160@61000):
total_amount = 150 + 160 = 310.
total_price_volume = 150*60000 + 160*61000 = 18,760,000.
avg_price = 18,760,000 / 310 ≈ 60516.129 → 60516.1.
Virtual trade: {ts: "2024-12-31T23:00:00Z", buy: true, price: 60516.1, amount: 310, trade_no: "buy_year_1:buy_year_2", flags: 2, ...}.






Sort and Output:

Sort virtual trades by ts (ascending).
Serialize to JSON array.
Example output:[
  {"ts": "2024-01-01T00:00:00Z", "buy": true, "price": 40000, "amount": 100, "trade_no": "buy_q1_1", "flags": 1, ...},
  {"ts": "2024-01-01T01:00:00Z", "buy": false, "price": 40500, "amount": 90, "trade_no": "sell_q1_1", "flags": 1, ...},
  ...
]





Notes

Preserves exact sequence of direction changes for accurate equity curve calculations.
Thread-safe with Arc<TradesCache>.

4. Expected Output Format
Both algorithms produce a JSON array where each virtual trade is:
{
  "ts": "YYYY-MM-DDThh:mm:ssZ", // RFC3339 timestamp
  "buy": true/false,
  "price": float,              // Rounded average price
  "amount": float,             // Rounded total amount
  "trade_no": string,          // "first:last" or single trade_no
  "pair_id": int,              // Inherited from input
  "flags": int,                // Number of source trades
  "order_id": 0,
  "position": 0.0,
  "rpnl": 0.0,
  "comission": 0.0
}

5. Edge Cases

Empty Trade List: Returns [].
Single Trade in Window/Group: Uses trade’s price, amount, trade_no, ts directly; flags=1.
Zero Amount: Sets price=0.0 to avoid division by zero.
Invalid Timestamps: Handled by TradesCache::import_csv with robust parsing (supports multiple formats, e.g., 2024-12-31T23:00:00Z, 2024-12-31 23:00:00+00:00).
Year Boundary: For 7d, week_align=true, trades on 2024-12-31 and 2025-01-01 align to 2024-12-30T00:00:00Z.
Large Intervals: For 365d, trades from entire year (e.g., 2024) aggregate into one window (e.g., 2024-01-01T00:00:00Z if week_align=false).

6. Implementation Details

File: src/entities/trades_aggregator.rs.
Rust Edition: 2021.
Dependencies:
chrono for timestamp handling.
serde_json for JSON serialization.
dashmap for thread-safe TradesCache.
tracing for logging (#DBG, #INFO).


Methods:
new: Initializes TradesAggregator with trades_cache, start_ts, end_ts, interval, calc_method, week_align.
aggregate_coarse: Implements Coarse algorithm.
aggregate_precise: Implements Precise algorithm.
aggregate_trades: Computes virtual trade for a group.


Integration: Runs as a Task in TaskProcessor (see src/services/task_processor.rs).
Tests:
src/tests/task/aggr_trades.rs: Tests 1h, 1d, 7d, 30d, 30d_weekly, 90d, 365d, precise against expected_results_coarse.json.
src/tests/interval_func.rs: Tests adjust_to_monday, adjust_to_first_of_month, adjust_to_first_of_quarter, adjust_to_first_of_year against interval_expected.json.
Example test cases:
7d: Verifies buy_year_1:buy_year_4 at 2024-12-30T00:00:00Z with price=61575.8, amount=660.
interval_func: Verifies 2024-12-31T23:59:59.999Z → 2024-12-30T00:00:00Z.





7. Example Workflow
For 7d, week_align=true, using sample_trades_extended.csv:

Input Trades:
buy_q1_1 (2024-01-01 00:00:00, 100@40000)
sell_q1_1 (2024-01-01 01:00:00, 90@40500)
buy_year_1 (2024-12-31 23:00:00, 150@60000)
buy_year_2 (2024-12-31 23:30:00, 160@61000)
...


Windows:
2024-01-01T00:00:00Z: buy_q1_1, sell_q1_1.
2024-12-30T00:00:00Z: buy_year_1, buy_year_2, buy_year_3, buy_year_4, sell_year_1, sell_year_2, sell_year_3, sell_year_4.


Aggregation:
Window 2024-12-30T00:00:00Z:
Buy: (150*60000 + 160*61000 + 170*62000 + 180*63000) / (150+160+170+180) ≈ 61575.76 → 61575.8.
Sell: (140*60500 + 150*61500 + 160*62500 + 170*63500) / (140+150+160+170) ≈ 62080.65 → 62080.6.




Output:[
  {"ts": "2024-01-01T00:00:00Z", "buy": true, "price": 40000, "amount": 100, "trade_no": "buy_q1_1", "flags": 1, ...},
  {"ts": "2024-01-01T00:00:00Z", "buy": false, "price": 40500, "amount": 90, "trade_no": "sell_q1_1", "flags": 1, ...},
  ...,
  {"ts": "2024-12-30T00:00:00Z", "buy": true, "price": 61575.8, "amount": 660, "trade_no": "buy_year_1:buy_year_4", "flags": 4, ...},
  {"ts": "2024-12-30T00:00:00Z", "buy": false, "price": 62080.6, "amount": 620, "trade_no": "sell_year_1:sell_year_4", "flags": 4, ...}
]



8. Validation

Tests: src/tests/task/aggr_trades.rs validates output against expected_results_coarse.json for all intervals, checking ts, buy, price, amount, trade_no, and window alignment.
Interval Tests: src/tests/interval_func.rs verifies alignment functions with cases like:
2024-12-31T23:59:59.999Z → 2024-12-30T00:00:00Z (adjust_to_monday).
2024-10-01T00:00:00Z → 2024-09-30T00:00:00Z (adjust_to_first_of_quarter, week_align=true).


Logging: Uses #DBG for trade grouping and #INFO for completion, per EDA Rule 4.
Thread-Safety: Ensured by Arc and DashMap in TradesCache and TaskProcessor.

9. Differences from Previous Versions

Added Details:
Numerical examples for 7d, 90d aggregation using sample_trades_extended.csv.
Clarified adjust_to_monday logic with hour-based loop and time reset.
Explained auto_round precision rules.
Included test references to aggr_trades.rs and interval_func.rs.


Preserved:
Core algorithm steps unchanged from previous version.
Alignment rules for week_align and monthly/quarterly/yearly intervals.
Output format and edge case handling.


Removed:
No significant content removed; only refined for clarity and added examples.



10. Future Improvements

Optimize adjust_to_monday for performance (e.g., use chrono::iso_week if loop-based approach is slow).
Add metrics for aggregation time in TaskProcessor.
Enhance tests for additional edge cases (e.g., invalid CSV formats).
Address parallel test failures (5 failing tests noted in CLA_selftest.md).
