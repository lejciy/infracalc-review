//! 全组合体系分析：知识库效率锚点 vs ArknightsInfraCalc-v3 评估函数核算。
//!
//! 对每个组合：按登记表构造「部分作业」（目标房 + 体系支撑房），锚度不足时
//! 用合成 operbox 提档评估；取评估口径字段与知识库锚点对照，产出核对结论：
//! match / approx / mismatch / lower_upper_bound / not_evaluable。

use crate::checks::LayoutInfo;
use crate::inputs::{room_kind, OperBox, PlanResult};
use crate::kb::Combo;
use crate::partial::{PartialEvaluator, RoomSpec};
use std::collections::HashMap;

/// 按目标产物解析实际落房：布局中找同设施类型且配方匹配的房间
/// （组合登记表里的 room_id 是示意编号，实际布局里产赤金的可能是 manu_3）。
/// 返回解析后的房间 id；解析不到时保留原 id。
fn resolve_room(layout: &LayoutInfo, want_product: Option<&str>, used: &mut Vec<String>) -> String {
    let Some(product) = want_product else {
        return String::new();
    };
    let (kind_prefix, recipe) = match product {
        "PureGold" => ("manu", Some("gold")),
        "BattleRecord" => ("manu", Some("battle_record")),
        "OriginiumShard" => ("manu", Some("originium")),
        "LMD" => ("trade", Some("gold")),
        _ => ("", None),
    };
    if kind_prefix.is_empty() {
        return String::new();
    }
    let mut ids: Vec<&String> = layout
        .rooms
        .keys()
        .filter(|id| id.starts_with(kind_prefix))
        .collect();
    ids.sort();
    for id in ids {
        if used.contains(id) {
            continue;
        }
        let matches_recipe = match (kind_prefix, recipe) {
            ("manu", Some(r)) => layout.manu_recipe(id) == r,
            ("trade", _) => true,
            _ => true,
        };
        if matches_recipe {
            used.push(id.clone());
            return id.clone();
        }
    }
    String::new()
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ComboCheck {
    pub id: String,
    pub name: String,
    pub kb_ref: String,
    pub metric: String,
    pub kb_value: f64,
    pub eval_value: Option<f64>,
    pub delta: Option<f64>,
    pub verdict: String,
    pub eval_rooms: Vec<EvalRoomEcho>,
    pub note: String,
    pub kb_note: String,
    pub mode: String,
    /// 评估只提交登记表中的房间；跨设施组合还可能需要宿舍、办公室、
    /// 其它产线或全局资源，不能把该结果当作完整排班证明。
    pub evaluation_scope: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EvalRoomEcho {
    pub room_id: String,
    pub kind: String,
    pub total: f64,
    pub final_efficiency: f64,
    pub skill: f64,
    pub global: f64,
    pub order_multiplier: f64,
    pub trade_equivalent: f64,
    pub gold_equivalent: f64,
    pub output_per_day: f64,
}

fn echo(room: &crate::partial::PartialRoomLine) -> EvalRoomEcho {
    EvalRoomEcho {
        room_id: room.room_id.clone(),
        kind: room.kind.clone(),
        total: room.total_efficiency,
        final_efficiency: room.final_efficiency,
        skill: room.skill,
        global: room.global,
        order_multiplier: room.order_multiplier,
        trade_equivalent: room.trade_equivalent_efficiency,
        gold_equivalent: room.gold_equivalent_efficiency,
        output_per_day: room
            .trade_output_per_day
            .max(room.manufacture_output_per_day),
    }
}

/// 评估一个组合并按锚点口径对照
pub fn check_combo(
    combo: &Combo,
    evaluator: &PartialEvaluator,
    box_data: &OperBox,
    layout: &LayoutInfo,
) -> ComboCheck {
    let mut out = ComboCheck {
        id: combo.id.clone(),
        name: combo.name.clone(),
        kb_ref: combo.kb_ref.clone(),
        metric: combo.anchor.metric.clone(),
        kb_value: combo.anchor.value,
        eval_value: None,
        delta: None,
        verdict: String::new(),
        eval_rooms: Vec::new(),
        note: String::new(),
        kb_note: combo.anchor.note.clone(),
        mode: if combo.anchor.mode.is_empty() {
            "exact".into()
        } else {
            combo.anchor.mode.clone()
        },
        evaluation_scope: if combo.layer.contains("跨设施") {
            "partial_cross_facility".into()
        } else {
            "partial_room".into()
        },
    };

    let Some(spec) = combo.eval.as_ref() else {
        out.verdict = "manual".into();
        out.note = combo.anchor.note.clone();
        return out;
    };

    // 持有检查（锚度覆盖不解决未持有）
    let missing: Vec<&str> = spec
        .rooms
        .iter()
        .flat_map(|r| r.operators.iter())
        .filter(|n| !box_data.owned(n))
        .map(|n| n.as_str())
        .collect();
    if !missing.is_empty() {
        out.verdict = "not_owned".into();
        out.note = format!("干员盒缺员：{}", missing.join("、"));
        return out;
    }

    // 构造房间规格：成员锚度覆盖（只升不降）；目标房按布局配方解析实际落房
    let member_tier: HashMap<&str, u8> = combo
        .members
        .iter()
        .map(|m| (m.name.as_str(), m.elite))
        .collect();
    let mut used_rooms: Vec<String> = Vec::new();
    let mut resolved_ids: Vec<String> = Vec::new();
    let mut rooms: Vec<RoomSpec> = Vec::new();
    for room in &spec.rooms {
        let mut overrides = HashMap::new();
        for name in &room.operators {
            if let Some(&elite) = member_tier.get(name.as_str()) {
                let current = box_data.get(name).map(|b| b.elite).unwrap_or(0);
                if elite > current {
                    overrides.insert(name.clone(), elite);
                }
            }
        }
        // 生产房按产物解析；control/power/dorm/office 等支撑房保留原 id
        let resolved = if room.product.is_some() {
            let id = resolve_room(layout, room.product.as_deref(), &mut used_rooms);
            if id.is_empty() {
                room.room_id.clone()
            } else {
                id
            }
        } else {
            room.room_id.clone()
        };
        resolved_ids.push(resolved.clone());
        rooms.push(RoomSpec {
            room_id: resolved,
            operators: room.operators.clone(),
            overrides,
        });
    }
    // 锚点房：显式下标优先，否则取规格序第一个生产房（触发房/支撑房不计入锚点）
    let anchor_room_id: String = spec
        .anchor_room
        .map(|i| resolved_ids.get(i).cloned().unwrap_or_default())
        .unwrap_or_else(|| {
            resolved_ids
                .iter()
                .find(|id| matches!(room_kind(id), "trade" | "manufacture"))
                .cloned()
                .unwrap_or_default()
        });

    // global_add 型：含/不含支撑中枢各评一次，取目标房 global 差值
    if combo.anchor.metric == "global_add" {
        let control_rooms: Vec<String> = spec
            .rooms
            .iter()
            .filter(|r| r.room_id == "control")
            .map(|r| r.room_id.clone())
            .collect();
        let target_kind = if combo.id == "mujica" {
            "trade"
        } else {
            "manufacture"
        };
        let with = evaluator.evaluate(&rooms, 24);
        let without_rooms: Vec<RoomSpec> = rooms
            .iter()
            .filter(|r| !control_rooms.contains(&r.room_id))
            .cloned()
            .collect();
        let without = evaluator.evaluate(&without_rooms, 24);
        match (with, without) {
            (Ok(w), Ok(wo)) => {
                let pick = |res: &crate::partial::PartialResult| {
                    res.rooms
                        .iter()
                        .find(|r| room_kind(&r.room_id) == target_kind)
                        .map(|r| r.global)
                        .unwrap_or(0.0)
                };
                let delta = pick(&w) - pick(&wo);
                out.eval_value = Some(delta);
                out.eval_rooms = w.rooms.iter().map(echo).collect();
                finish(&mut out);
                out
            }
            (Err(e), _) | (_, Err(e)) => {
                out.verdict = "eval_failed".into();
                out.note = e;
                out
            }
        }
    } else {
        match evaluator.evaluate(&rooms, 24) {
            Ok(res) => {
                // 取目标房间（贸易/制造；办公室/会客室取 pct）
                let value = if combo.anchor.metric == "pct" {
                    res.rooms
                        .iter()
                        .find(|r| {
                            room_kind(&r.room_id) == "office" || room_kind(&r.room_id) == "meeting"
                        })
                        .map(|r| r.total_efficiency)
                } else {
                    // 生产房序列：默认取首个生产房（触发房/支撑房不计入锚点口径）；
                    // aggregate="sum" 时取全部生产房合计（如深海 2+2）
                    let prod: Vec<&crate::partial::PartialRoomLine> = res
                        .rooms
                        .iter()
                        .filter(|r| {
                            let k = room_kind(&r.room_id);
                            k == "trade" || k == "manufacture"
                        })
                        .collect();
                    match combo.anchor.metric.as_str() {
                        "skill_paper" if spec.aggregate == "sum" => {
                            Some(prod.iter().map(|r| r.skill).sum::<f64>())
                        }
                        _ => prod
                            .iter()
                            .find(|r| r.room_id == anchor_room_id)
                            .or_else(|| prod.first())
                            .map(|r| match combo.anchor.metric.as_str() {
                                "order_multiplier" => r.order_multiplier,
                                "equivalent_trade" => r.trade_equivalent_efficiency,
                                "equivalent_gold" => r.gold_equivalent_efficiency,
                                _ => r.skill,
                            }),
                    }
                };
                out.eval_value = value;
                out.eval_rooms = res.rooms.iter().map(echo).collect();
                finish(&mut out);
                out
            }
            Err(e) => {
                out.verdict = "eval_failed".into();
                out.note = e;
                out
            }
        }
    }
}

fn finish(out: &mut ComboCheck) {
    let (Some(ev), kb) = (out.eval_value, out.kb_value) else {
        out.verdict = "eval_failed".into();
        return;
    };
    out.delta = Some(ev - kb);
    let d = (ev - kb).abs();
    out.verdict = if d <= 0.02 || (kb != 0.0 && d / kb <= 0.03) {
        "match".into()
    } else if d <= 0.08 || (kb != 0.0 && d / kb <= 0.08) {
        "approx".into()
    } else if out.mode == "upper_bound" && ev < kb {
        "below_upper_bound".into()
    } else if out.mode == "upper_bound" && ev >= kb {
        "exceeds_kb".into()
    } else {
        "mismatch".into()
    };
}

/// 对排班表本身做逐班复评（同一评估函数、作业来自 rotation），与 plan.compute
/// 自报的逐房效率对照——「基于知识库描述与基于评估函数的效率表现核对」的排班侧。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ShiftCrossCheck {
    pub shift: usize,
    pub room_id: String,
    pub plan_total: f64,
    pub eval_total: f64,
    pub plan_skill: f64,
    pub eval_skill: f64,
    pub plan_global: f64,
    pub eval_global: f64,
    pub total_delta: f64,
    pub status: String,
}

pub fn cross_check_shifts(
    result: &PlanResult,
    evaluator: &PartialEvaluator,
) -> Result<Vec<ShiftCrossCheck>, String> {
    let mut out = Vec::new();
    for shift in &result.rotation.shifts {
        // 作业房间：生产+中枢+发电+会客+办公室（宿舍为休息位，评估意义不同，跳过）
        let rooms: Vec<RoomSpec> = shift
            .assignment
            .rooms
            .iter()
            .filter(|r| !r.room_id.starts_with("dorm"))
            .map(|r| RoomSpec {
                room_id: r.room_id.clone(),
                operators: r.operators.iter().map(|o| o.name.clone()).collect(),
                overrides: HashMap::new(),
            })
            .collect();
        let evaluated = evaluator.evaluate(&rooms, shift.duration_hours.max(1.0) as u32)?;
        for line in &shift.efficiencies.room_lines {
            let kind = room_kind(&line.room_id);
            if kind != "trade" && kind != "manufacture" && kind != "power" {
                continue;
            }
            if let Some(ev) = evaluated.rooms.iter().find(|er| er.room_id == line.room_id) {
                out.push(ShiftCrossCheck {
                    shift: shift.index,
                    room_id: line.room_id.clone(),
                    plan_total: line.total_efficiency,
                    eval_total: ev.total_efficiency,
                    plan_skill: if kind == "trade" {
                        line.operator_efficiency
                    } else {
                        line.manufacture_skill_efficiency
                    },
                    eval_skill: ev.skill,
                    plan_global: line.global_efficiency,
                    eval_global: ev.global,
                    total_delta: ev.total_efficiency - line.total_efficiency,
                    status: if (ev.total_efficiency - line.total_efficiency).abs() <= 0.005 {
                        "match".into()
                    } else {
                        "review".into()
                    },
                });
            }
        }
    }
    Ok(out)
}
