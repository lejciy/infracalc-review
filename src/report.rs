//! 报告输出：JSON 落盘 + Markdown 摘要。

use crate::analyze::{ComboCheck, ShiftCrossCheck};
use crate::checks::Finding;
use serde::Serialize;

#[derive(Serialize)]
pub struct ReviewReport {
    pub tool: String,
    pub generated_at: String,
    pub layout: String,
    pub summary: ReviewSummary,
    pub findings: Vec<Finding>,
    pub combo_checks: Option<Vec<ComboCheck>>,
    pub shift_cross_checks: Option<Vec<ShiftCrossCheck>>,
}

#[derive(Serialize, Default)]
pub struct ReviewSummary {
    pub errors: usize,
    pub improvements: usize,
    pub notes: usize,
    pub combo_match: usize,
    pub combo_approx: usize,
    pub combo_mismatch: usize,
    pub combo_other: usize,
}

pub fn count_summary(findings: &[Finding], combos: Option<&[ComboCheck]>) -> ReviewSummary {
    let mut s = ReviewSummary::default();
    for f in findings {
        match f.severity.as_str() {
            "error" => s.errors += 1,
            "improvement" => s.improvements += 1,
            _ => s.notes += 1,
        }
    }
    if let Some(list) = combos {
        for c in list {
            match c.verdict.as_str() {
                "match" => s.combo_match += 1,
                "approx" => s.combo_approx += 1,
                "mismatch" | "exceeds_kb" => s.combo_mismatch += 1,
                _ => s.combo_other += 1,
            }
        }
    }
    s
}

pub fn markdown(report: &ReviewReport) -> String {
    let mut md = String::new();
    md.push_str("# 排班表审查报告（infracalc-review）\n\n");
    md.push_str(&format!(
        "- 布局：{}\n- 明确错误：{} 项｜需要改进：{} 项｜提示：{} 项\n",
        report.layout, report.summary.errors, report.summary.improvements, report.summary.notes
    ));
    if report.combo_checks.is_some() {
        md.push_str(&format!(
            "- 组合核对：match {}｜approx {}｜mismatch {}｜其他 {}（not_owned/manual/eval_failed）\n",
            report.summary.combo_match,
            report.summary.combo_approx,
            report.summary.combo_mismatch,
            report.summary.combo_other
        ));
    }
    md.push_str("\n## 明确错误的点\n\n");
    let errors: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.severity == "error")
        .collect();
    if errors.is_empty() {
        md.push_str("（无）\n");
    }
    for f in errors {
        md.push_str(&format!(
            "- [检查{}] {}{}：{}（{}）\n",
            f.check,
            f.shift.map(|s| format!("班{} ", s + 1)).unwrap_or_default(),
            f.room.clone().unwrap_or_default(),
            f.message,
            f.kb_ref
        ));
    }
    md.push_str("\n## 需要改进的点\n\n");
    let improvements: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.severity == "improvement")
        .collect();
    if improvements.is_empty() {
        md.push_str("（无）\n");
    }
    for f in improvements {
        md.push_str(&format!(
            "- [检查{}] {}{}：{}（{}）\n",
            f.check,
            f.shift.map(|s| format!("班{} ", s + 1)).unwrap_or_default(),
            f.room.clone().unwrap_or_default(),
            f.message,
            f.kb_ref
        ));
    }
    let notes: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.severity == "note")
        .collect();
    if !notes.is_empty() {
        md.push_str("\n## 提示\n\n");
        for f in notes {
            md.push_str(&format!(
                "- [检查{}] {}：{}（{}）\n",
                f.check,
                f.room.clone().unwrap_or_default(),
                f.message,
                f.kb_ref
            ));
        }
    }
    if let Some(combos) = &report.combo_checks {
        md.push_str("\n## 组合体系核对：知识库锚点 vs 评估函数\n\n");
        md.push_str(
            "| 组合 | 口径 | 范围 | 知识库 | 评估 | 差值 | 结论 |\n|---|---|---|---|---|---|---|\n",
        );
        for c in combos {
            md.push_str(&format!(
                "| {} | {} | {} | {:.3} | {} | {} | {} |\n",
                c.name,
                c.metric,
                c.evaluation_scope,
                c.kb_value,
                c.eval_value
                    .map(|v| format!("{v:.3}"))
                    .unwrap_or_else(|| "—".into()),
                c.delta
                    .map(|v| format!("{v:+.3}"))
                    .unwrap_or_else(|| "—".into()),
                c.verdict
            ));
        }
        md.push_str("\n说明：match=差值≤2%（或 0.02）；approx≤8%；mismatch>8%（知识库或评估器口径需人工复核）；below_upper_bound=部分评估只覆盖子集，低于满配锚点属预期。`partial_cross_facility` 表示只提交组合登记的房间，未自动补入其它全基建上下文。\n");
        for c in combos.iter().filter(|c| !c.note.is_empty()) {
            md.push_str(&format!("- **{}**：{}\n", c.name, c.note));
        }
    }
    if let Some(cross) = &report.shift_cross_checks {
        md.push_str("\n## 排班表逐班复评（rotation vs 本工具评估）\n\n| 班次 | 房间 | plan total | eval total | 差值 | plan skill | eval skill | plan global | eval global | 状态 |\n|---|---|---|---|---:|---|---|---|---|---|\n");
        for c in cross {
            md.push_str(&format!(
                "| {} | {} | {:.3} | {:.3} | {:+.3} | {:.3} | {:.3} | {:.3} | {:.3} | {} |\n",
                c.shift + 1,
                c.room_id,
                c.plan_total,
                c.eval_total,
                c.total_delta,
                c.plan_skill,
                c.eval_skill,
                c.plan_global,
                c.eval_global,
                c.status
            ));
        }
    }
    md
}

pub fn now_string() -> String {
    // 无 chrono 依赖：取系统秒级时间戳足够
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}
