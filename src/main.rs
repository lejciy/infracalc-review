//! infracalc-review：排班表审查与组合效率核对工具。
//!
//! 用法（在 infracalc-review 目录）：
//!   infracalc-review review  --layout <layout.json> --box <box.json> --plan <plan-compute.json> [--out r.json] [--md r.md]
//!   infracalc-review combos  --layout <layout.json> --box <box.json> [--out c.json] [--md c.md] [--only <id>]
//!   infracalc-review room    --layout <layout.json> --box <box.json> --room trade_1 --ops 巫恋,龙舌兰,卡夫卡 [--hours 24]
//!
//! 依赖 ArknightsInfraCalc-v3（path 依赖 ../ArknightsInfraCalc-v3）的效率评估函数，
//! 部分求解改造见 partial.rs。

mod analyze;
mod checks;
mod inputs;
mod kb;
mod partial;
mod report;

use std::collections::HashMap;

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|p| p[0] == name).map(|p| p[1].clone())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        return;
    }
    let result = match args[0].as_str() {
        "review" => cmd_review(&args[1..]),
        "combos" => cmd_combos(&args[1..]),
        "room" => cmd_room(&args[1..]),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown command {other:?}")),
    };
    if let Err(message) = result {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  infracalc-review review --layout <layout.json> --box <box.json> --plan <plan-compute.json> [--out r.json] [--md r.md]");
    eprintln!("  infracalc-review combos --layout <layout.json> --box <box.json> [--out c.json] [--md c.md] [--only <id>]");
    eprintln!("  infracalc-review room   --layout <layout.json> --box <box.json> --room <room_id> --ops <A,B,C> [--hours 24]");
}

fn cmd_review(args: &[String]) -> Result<(), String> {
    let layout_path = arg_value(args, "--layout").ok_or("missing --layout")?;
    let box_path = arg_value(args, "--box").ok_or("missing --box")?;
    let plan_path = arg_value(args, "--plan").ok_or("missing --plan")?;
    let plan = inputs::PlanEnvelope::load(&plan_path)?;
    let box_data = inputs::OperBox::load(&box_path)?;
    let layout = checks::LayoutInfo::load(&layout_path)?;
    let registry = kb::load_registry()?;

    let mut findings = Vec::new();
    if !plan.ok {
        findings.push(checks::protocol_finding(
            "plan.compute 返回 ok=false；该报告只审查失败协议和可用字段",
        ));
    }
    let Some(result) = plan.result.as_ref() else {
        if let Some(error) = &plan.error {
            findings.push(checks::protocol_finding(&format!(
                "plan.compute error: {error}"
            )));
        } else {
            findings.push(checks::protocol_finding(
                "plan.compute 失败响应缺少 error 字段",
            ));
        }
        let report = report::ReviewReport {
            tool: "infracalc-review".into(),
            generated_at: report::now_string(),
            layout: layout.template.clone(),
            summary: report::count_summary(&findings, None),
            findings,
            combo_checks: None,
            shift_cross_checks: None,
        };
        emit(report, args);
        return Ok(());
    };
    if !(1..=4).contains(&result.schema_version) {
        findings.push(checks::protocol_finding(&format!(
            "不支持的 plan result.schema_version={}（契约范围 1–4）",
            result.schema_version
        )));
    }
    findings.extend(checks::run_all_checks(
        result, &box_data, &layout, &registry,
    ));

    // 逐班复评 + 组合核对
    let evaluator = partial::PartialEvaluator::new(&layout_path, &box_path)?;
    let cross = match analyze::cross_check_shifts(result, &evaluator) {
        Ok(cross) => Some(cross),
        Err(error) => {
            findings.push(checks::protocol_finding(&format!(
                "逐班复评未完成：{error}"
            )));
            None
        }
    };
    let mut combo_checks = Vec::new();
    for combo in &registry.combos {
        combo_checks.push(analyze::check_combo(combo, &evaluator, &box_data, &layout));
    }

    let report = report::ReviewReport {
        tool: "infracalc-review".into(),
        generated_at: report::now_string(),
        layout: layout.template.clone(),
        summary: report::count_summary(&findings, Some(&combo_checks)),
        findings,
        combo_checks: Some(combo_checks),
        shift_cross_checks: cross,
    };
    emit(report, args);
    Ok(())
}

fn cmd_combos(args: &[String]) -> Result<(), String> {
    let layout_path = arg_value(args, "--layout").ok_or("missing --layout")?;
    let box_path = arg_value(args, "--box").ok_or("missing --box")?;
    let box_data = inputs::OperBox::load(&box_path)?;
    let layout = checks::LayoutInfo::load(&layout_path)?;
    let registry = kb::load_registry()?;
    let evaluator = partial::PartialEvaluator::new(&layout_path, &box_path)?;
    let only = arg_value(args, "--only");

    let mut combo_checks = Vec::new();
    for combo in &registry.combos {
        if let Some(id) = &only {
            if &combo.id != id && !combo.aliases.iter().any(|a| a == id) && combo.name != *id {
                continue;
            }
        }
        combo_checks.push(analyze::check_combo(combo, &evaluator, &box_data, &layout));
    }
    let report = report::ReviewReport {
        tool: "infracalc-review".into(),
        generated_at: report::now_string(),
        layout: layout.template.clone(),
        summary: report::count_summary(&[], Some(&combo_checks)),
        findings: Vec::new(),
        combo_checks: Some(combo_checks),
        shift_cross_checks: None,
    };
    emit(report, args);
    Ok(())
}

fn cmd_room(args: &[String]) -> Result<(), String> {
    let layout_path = arg_value(args, "--layout").ok_or("missing --layout")?;
    let box_path = arg_value(args, "--box").ok_or("missing --box")?;
    let room_id = arg_value(args, "--room").ok_or("missing --room")?;
    let ops = arg_value(args, "--ops").ok_or("missing --ops")?;
    let hours: u32 = arg_value(args, "--hours")
        .and_then(|h| h.parse().ok())
        .unwrap_or(24);
    let evaluator = partial::PartialEvaluator::new(&layout_path, &box_path)?;
    let names: Vec<String> = ops.split(',').map(|s| s.trim().to_string()).collect();
    let result = evaluator.evaluate(
        &[partial::RoomSpec {
            room_id,
            operators: names,
            overrides: HashMap::new(),
        }],
        hours,
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&result.rooms).unwrap_or_default()
    );
    Ok(())
}

fn emit(report: report::ReviewReport, args: &[String]) {
    let json_path = arg_value(args, "--out").unwrap_or_else(|| "report.json".into());
    let md_path = arg_value(args, "--md");
    if let Some(parent) = std::path::Path::new(&json_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(&report).unwrap_or_default();
    if let Err(e) = std::fs::write(&json_path, &json) {
        eprintln!("write {json_path}: {e}");
    }
    println!("json  -> {json_path}");
    let summary = &report.summary;
    println!(
        "summary: errors={} improvements={} notes={} | combos match={} approx={} mismatch={} other={}",
        summary.errors,
        summary.improvements,
        summary.notes,
        summary.combo_match,
        summary.combo_approx,
        summary.combo_mismatch,
        summary.combo_other
    );
    if let Some(md_path) = md_path {
        let md = report::markdown(&report);
        std::fs::write(&md_path, md).ok();
        println!("md     -> {md_path}");
    }
}
