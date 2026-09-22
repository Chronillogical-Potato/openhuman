use super::*;
use chrono::{Datelike, Duration};
use tempfile::TempDir;

fn enabled_config() -> CostConfig {
    CostConfig {
        enabled: true,
        ..Default::default()
    }
}

/// A managed-backend tier slug — spend on this route is billed to OpenHuman
/// credits and so is the only kind the local budget may gate (#5016).
const MANAGED_MODEL: &str = "chat-v1";

/// A bring-your-own-key model id, as reported in #5016 (OpenRouter). Spend
/// here is billed by the user's own provider and must never gate a request.
const BYOK_MODEL: &str = "minimax/minimax-m3";

#[test]
fn cost_tracker_initialization() {
    let tmp = TempDir::new().unwrap();
    let tracker = CostTracker::new(enabled_config(), tmp.path()).unwrap();
    assert!(!tracker.session_id().is_empty());
}

#[test]
fn record_usage_and_get_summary() {
    let tmp = TempDir::new().unwrap();
    let tracker = CostTracker::new(enabled_config(), tmp.path()).unwrap();

    let usage = TokenUsage::new("test/model", 1000, 500, 1.0, 2.0);
    tracker.record_usage(usage).unwrap();

    let summary = tracker.get_summary().unwrap();
    assert_eq!(summary.request_count, 1);
    assert!(summary.session_cost_usd > 0.0);
    assert_eq!(summary.by_model.len(), 1);
}

#[test]
fn summary_by_model_is_session_scoped() {
    let tmp = TempDir::new().unwrap();
    let storage_path = resolve_storage_path(tmp.path()).unwrap();
    if let Some(parent) = storage_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }

    let old_record = CostRecord::new(
        "old-session",
        TokenUsage::new("legacy/model", 500, 500, 1.0, 1.0),
    );
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(storage_path)
        .unwrap();
    writeln!(file, "{}", serde_json::to_string(&old_record).unwrap()).unwrap();
    file.sync_all().unwrap();

    let tracker = CostTracker::new(enabled_config(), tmp.path()).unwrap();
    tracker
        .record_usage(TokenUsage::new("session/model", 1000, 1000, 1.0, 1.0))
        .unwrap();

    let summary = tracker.get_summary().unwrap();
    assert_eq!(summary.by_model.len(), 1);
    assert!(summary.by_model.contains_key("session/model"));
    assert!(!summary.by_model.contains_key("legacy/model"));
}

#[test]
fn malformed_lines_are_ignored_while_loading() {
    let tmp = TempDir::new().unwrap();
    let storage_path = resolve_storage_path(tmp.path()).unwrap();
    if let Some(parent) = storage_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }

    let valid_usage = TokenUsage::new("test/model", 1000, 0, 1.0, 1.0);
    let valid_record = CostRecord::new("session-a", valid_usage.clone());

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(storage_path)
        .unwrap();
    writeln!(file, "{}", serde_json::to_string(&valid_record).unwrap()).unwrap();
    writeln!(file, "not-a-json-line").unwrap();
    writeln!(file).unwrap();
    file.sync_all().unwrap();

    let tracker = CostTracker::new(enabled_config(), tmp.path()).unwrap();
    let today_cost = tracker.get_daily_cost(Utc::now().date_naive()).unwrap();
    assert!((today_cost - valid_usage.cost_usd).abs() < f64::EPSILON);
}

#[test]
fn record_usage_when_disabled_is_noop() {
    let tmp = TempDir::new().unwrap();
    let config = CostConfig {
        enabled: false,
        ..Default::default()
    };
    let tracker = CostTracker::new(config, tmp.path()).unwrap();
    let usage = TokenUsage::new("test/model", 1000, 500, 1.0, 2.0);
    tracker.record_usage(usage).unwrap();
    let summary = tracker.get_summary().unwrap();
    assert_eq!(summary.request_count, 0);
}

#[test]
fn record_usage_unconditional_bypasses_disabled_gate() {
    let tmp = TempDir::new().unwrap();
    let config = CostConfig {
        enabled: false,
        ..Default::default()
    };
    let tracker = CostTracker::new(config, tmp.path()).unwrap();
    let usage = TokenUsage::new("test/model", 1000, 500, 1.0, 2.0);
    tracker.record_usage_unconditional(usage.clone()).unwrap();
    let summary = tracker.get_summary().unwrap();
    assert_eq!(summary.request_count, 1);
    let today_cost = tracker.get_daily_cost(Utc::now().date_naive()).unwrap();
    assert!((today_cost - usage.cost_usd).abs() < f64::EPSILON);
}

#[test]
fn record_usage_rejects_negative_cost() {
    let tmp = TempDir::new().unwrap();
    let tracker = CostTracker::new(enabled_config(), tmp.path()).unwrap();
    let mut usage = TokenUsage::new("test/model", 1000, 500, 1.0, 2.0);
    usage.cost_usd = -1.0;
    assert!(tracker.record_usage(usage).is_err());
}

#[test]
fn record_usage_rejects_nan_cost() {
    let tmp = TempDir::new().unwrap();
    let tracker = CostTracker::new(enabled_config(), tmp.path()).unwrap();
    let mut usage = TokenUsage::new("test/model", 1000, 500, 1.0, 2.0);
    usage.cost_usd = f64::NAN;
    assert!(tracker.record_usage(usage).is_err());
}

#[test]
fn byok_spend_is_still_recorded_for_the_dashboard() {
    // Exempting BYOK from the *budget* must not hide it from usage reporting:
    // the user in #5016 explicitly wanted to understand the counter.
    let tmp = TempDir::new().unwrap();
    let tracker = CostTracker::new(enabled_config(), tmp.path()).unwrap();

    let mut usage = TokenUsage::new(BYOK_MODEL, 1000, 500, 1.0, 1.0);
    usage.cost_usd = 4.25;
    tracker.record_usage(usage).unwrap();

    let summary = tracker.get_summary().unwrap();
    assert_eq!(summary.request_count, 1);
    assert!((summary.daily_cost_usd - 4.25).abs() < 0.0001);
    assert!((summary.session_cost_usd - 4.25).abs() < 0.0001);
}

#[test]
fn legacy_byok_records_are_exempt_after_an_aggregate_rebuild() {
    // Records persisted by builds that predate #5016 carry no route field. The
    // route is derived from the model id they already store, so a tracker that
    // rebuilds its aggregates from disk classifies them correctly with no
    // migration — this is what unblocks an affected user on upgrade rather
    // than making them wait out the window.
    let tmp = TempDir::new().unwrap();
    let today = Utc::now();
    write_raw_record(tmp.path(), &dated_record("legacy", BYOK_MODEL, 50.0, today));

    let config = CostConfig {
        enabled: true,
        monthly_limit_usd: 10.0,
        ..Default::default()
    };
    let tracker = CostTracker::new(config, tmp.path()).unwrap();

    // Nothing refuses a request on cost any more, so what matters on upgrade is
    // that the legacy rows are classified correctly: they show up in the usage
    // figures without inflating the managed totals the dashboard is drawn
    // against.
    let now = Utc::now();
    let monthly = tracker.get_monthly_cost(now.year(), now.month()).unwrap();
    assert!((monthly - 50.0).abs() < 0.0001);
    let managed_monthly = tracker
        .get_managed_monthly_cost(now.year(), now.month())
        .unwrap();
    assert!(managed_monthly.abs() < f64::EPSILON);
}

#[test]
fn dashboard_budget_gauge_reflects_managed_spend_only() {
    // The phantom "$10/day limit" in the issue was also visible as a budget
    // gauge filling up from BYOK spend against a cap that could never fire.
    let tmp = TempDir::new().unwrap();
    let today = Utc::now();
    write_raw_record(tmp.path(), &dated_record("s1", BYOK_MODEL, 95.0, today));

    let config = CostConfig {
        enabled: true,
        monthly_limit_usd: 100.0,
        ..Default::default()
    };
    let tracker = CostTracker::new(config, tmp.path()).unwrap();
    let dash = tracker.get_dashboard("USD", 0.8, 0.95).unwrap();

    // Usage is still reported…
    assert!((dash.month_to_date_usd - 95.0).abs() < 0.0001);
    assert!((dash.period_total_usd - 95.0).abs() < 0.0001);
    // …but the budget gauge stays empty, because none of it is gateable.
    assert_eq!(dash.budget_status, BudgetStatus::Normal);
    assert!(dash.budget_utilization.abs() < f64::EPSILON);
}

#[test]
fn get_daily_cost_for_today() {
    let tmp = TempDir::new().unwrap();
    let tracker = CostTracker::new(enabled_config(), tmp.path()).unwrap();
    let usage = TokenUsage::new("test/model", 1000, 500, 1.0, 2.0);
    tracker.record_usage(usage.clone()).unwrap();

    let today_cost = tracker.get_daily_cost(Utc::now().date_naive()).unwrap();
    assert!((today_cost - usage.cost_usd).abs() < 0.001);
}

#[test]
fn get_monthly_cost_for_current_month() {
    let tmp = TempDir::new().unwrap();
    let tracker = CostTracker::new(enabled_config(), tmp.path()).unwrap();
    let usage = TokenUsage::new("test/model", 1000, 500, 1.0, 2.0);
    tracker.record_usage(usage.clone()).unwrap();

    let now = Utc::now();
    let monthly_cost = tracker.get_monthly_cost(now.year(), now.month()).unwrap();
    assert!((monthly_cost - usage.cost_usd).abs() < 0.001);
}

fn write_raw_record(workspace: &Path, record: &CostRecord) {
    let storage_path = resolve_storage_path(workspace).unwrap();
    if let Some(parent) = storage_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(storage_path)
        .unwrap();
    writeln!(file, "{}", serde_json::to_string(record).unwrap()).unwrap();
    file.sync_all().unwrap();
}

fn dated_record(session: &str, model: &str, cost: f64, when: chrono::DateTime<Utc>) -> CostRecord {
    let mut usage = TokenUsage::new(model, 1000, 500, 1.0, 1.0);
    usage.cost_usd = cost;
    usage.timestamp = when;
    CostRecord::new(session, usage)
}

#[test]
fn get_daily_history_returns_seven_days_with_gaps_filled() {
    let tmp = TempDir::new().unwrap();
    let today = Utc::now();
    let three_days_ago = today - Duration::days(3);
    let six_days_ago = today - Duration::days(6);

    write_raw_record(
        tmp.path(),
        &dated_record("s1", "model-a", 1.50, three_days_ago),
    );
    write_raw_record(
        tmp.path(),
        &dated_record("s1", "model-b", 0.50, six_days_ago),
    );

    let tracker = CostTracker::new(enabled_config(), tmp.path()).unwrap();
    let history = tracker.get_daily_history(7).unwrap();
    assert_eq!(history.len(), 7);
    // Oldest first → six_days_ago
    assert_eq!(history[0].date, six_days_ago.date_naive());
    assert!((history[0].cost_usd - 0.50).abs() < f64::EPSILON);
    // Three days ago has the second record
    assert_eq!(history[3].date, three_days_ago.date_naive());
    assert!((history[3].cost_usd - 1.50).abs() < f64::EPSILON);
    // Today is the last bucket
    assert_eq!(history[6].date, today.date_naive());
    assert!(history[6].cost_usd.abs() < f64::EPSILON);
    assert_eq!(history[6].request_count, 0);
}

#[test]
fn get_daily_history_excludes_out_of_window_records() {
    let tmp = TempDir::new().unwrap();
    let today = Utc::now();
    let ten_days_ago = today - Duration::days(10);
    write_raw_record(
        tmp.path(),
        &dated_record("s1", "model-a", 99.0, ten_days_ago),
    );

    let tracker = CostTracker::new(enabled_config(), tmp.path()).unwrap();
    let history = tracker.get_daily_history(7).unwrap();
    assert_eq!(history.len(), 7);
    let total: f64 = history.iter().map(|e| e.cost_usd).sum();
    assert!(total.abs() < f64::EPSILON);
}

#[test]
fn get_daily_history_clamps_days_argument() {
    let tmp = TempDir::new().unwrap();
    let tracker = CostTracker::new(enabled_config(), tmp.path()).unwrap();
    assert_eq!(tracker.get_daily_history(0).unwrap().len(), 1);
    assert_eq!(tracker.get_daily_history(367).unwrap().len(), 366);
}

#[test]
fn get_dashboard_computes_period_total_and_monthly_pace() {
    let tmp = TempDir::new().unwrap();
    let today = Utc::now();
    write_raw_record(tmp.path(), &dated_record("s1", "model-a", 2.0, today));
    write_raw_record(
        tmp.path(),
        &dated_record("s1", "model-b", 0.5, today - Duration::days(1)),
    );

    let config = CostConfig {
        enabled: true,
        monthly_limit_usd: 100.0,
        ..Default::default()
    };
    let tracker = CostTracker::new(config, tmp.path()).unwrap();
    let dash = tracker.get_dashboard("USD", 0.8, 0.95).unwrap();
    assert_eq!(dash.days.len(), 7);
    assert!((dash.period_total_usd - 2.5).abs() < 0.0001);
    // daily avg = 2.5/7, monthly pace = avg * 30
    let expected_pace = (2.5 / 7.0) * 30.0;
    assert!((dash.monthly_pace_usd - expected_pace).abs() < 0.0001);
    assert_eq!(dash.currency, "USD");
    // 2.5 spend on 100 budget → 2.5% utilisation, well below 80% warn.
    assert_eq!(dash.budget_status, BudgetStatus::Normal);
}

#[test]
fn get_dashboard_budget_status_warning_and_exceeded() {
    let tmp = TempDir::new().unwrap();
    let today = Utc::now();
    // Managed-route spend: the budget gauge only tracks what can be gated
    // (#5016), so these have to be managed tier ids to move the status.
    write_raw_record(tmp.path(), &dated_record("s1", MANAGED_MODEL, 85.0, today));

    let config = CostConfig {
        enabled: true,
        monthly_limit_usd: 100.0,
        ..Default::default()
    };
    let tracker = CostTracker::new(config.clone(), tmp.path()).unwrap();
    let warn_dash = tracker.get_dashboard("USD", 0.8, 0.95).unwrap();
    assert_eq!(warn_dash.budget_status, BudgetStatus::Warning);

    write_raw_record(tmp.path(), &dated_record("s1", MANAGED_MODEL, 15.0, today));
    let tracker2 = CostTracker::new(config, tmp.path()).unwrap();
    let alert_dash = tracker2.get_dashboard("USD", 0.8, 0.95).unwrap();
    assert_eq!(alert_dash.budget_status, BudgetStatus::Exceeded);
    assert!((alert_dash.budget_utilization - 1.0).abs() < f64::EPSILON);
}

#[test]
fn get_dashboard_budget_status_normal_when_limit_zero() {
    let tmp = TempDir::new().unwrap();
    let config = CostConfig {
        enabled: true,
        monthly_limit_usd: 0.0,
        ..Default::default()
    };
    let tracker = CostTracker::new(config, tmp.path()).unwrap();
    let dash = tracker.get_dashboard("USD", 0.8, 0.95).unwrap();
    assert_eq!(dash.budget_status, BudgetStatus::Normal);
    assert!(dash.budget_utilization.abs() < f64::EPSILON);
}

#[test]
fn get_dashboard_by_model_is_sorted_desc() {
    let tmp = TempDir::new().unwrap();
    let today = Utc::now();
    write_raw_record(tmp.path(), &dated_record("s1", "model-a", 1.0, today));
    write_raw_record(tmp.path(), &dated_record("s1", "model-b", 5.0, today));
    write_raw_record(tmp.path(), &dated_record("s1", "model-c", 3.0, today));

    let tracker = CostTracker::new(enabled_config(), tmp.path()).unwrap();
    let dash = tracker.get_dashboard("USD", 0.8, 0.95).unwrap();
    assert_eq!(dash.by_model.len(), 3);
    assert_eq!(dash.by_model[0].model, "model-b");
    assert_eq!(dash.by_model[1].model, "model-c");
    assert_eq!(dash.by_model[2].model, "model-a");
}

#[test]
fn build_session_model_stats_aggregates_correctly() {
    let records = vec![
        CostRecord::new("s1", TokenUsage::new("model-a", 100, 50, 1.0, 1.0)),
        CostRecord::new("s1", TokenUsage::new("model-a", 200, 100, 1.0, 1.0)),
        CostRecord::new("s1", TokenUsage::new("model-b", 300, 150, 1.0, 1.0)),
    ];
    let stats = build_session_model_stats(&records);
    assert_eq!(stats.len(), 2);
    assert_eq!(stats["model-a"].request_count, 2);
    assert_eq!(stats["model-a"].total_tokens, 450);
    assert_eq!(stats["model-b"].request_count, 1);
}
