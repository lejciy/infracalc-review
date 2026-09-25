//! 部分求解评估器：把「部分干员 / 组合 / 体系」构造成固定作业，调用
//! ArknightsInfraCalc-v3 的效率评估函数结算，并支持锚度覆盖与心情注入。
//!
//! 改造点（相对 v3 的 `eval` CLI 全量口径）：
//! 1. 只提交目标房间——其余房间留空，实现子集求解；
//! 2. 锚度覆盖：组合成员可按知识库锚点练度评估（合成 operbox 提升精英化档），
//!    解决「box 练度低于组合档位时无法核对该档效率」的问题；
//! 3. `work_mood` 注入：默认满心情 24 评估（与知识库锚点口径一致）。

use crate::inputs::room_kind;
use arknights_infra_v3::compat::{
    eval_v2_assignment_with_shift, NormalizedBlueprint, V2AssignedOperator, V2Assignment,
    V2RoomAssignment,
};
use arknights_infra_v3::operbox::{BoxInputMode, BoxState};
use std::collections::{HashMap, HashSet};

pub struct PartialEvaluator {
    pub blueprint: NormalizedBlueprint,
    pub box_json: serde_json::Value,
    pub box_state: BoxState,
    /// 冷路径的姓名 → 身份快照；结算热路径仍由 v3 的整数状态负责。
    box_identity: HashMap<String, (u8, u32, u8, bool)>,
}

/// 单个目标房间的构造规格
#[derive(Clone)]
pub struct RoomSpec {
    pub room_id: String,
    pub operators: Vec<String>,
    /// 锚度覆盖：名字 → elite；等级始终沿用 box 的实际 `level`。
    pub overrides: HashMap<String, u8>,
}

/// 一次部分评估的结果（只保留目标房间行）
pub struct PartialResult {
    pub rooms: Vec<PartialRoomLine>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PartialRoomLine {
    pub room_id: String,
    pub kind: String,
    pub level: u8,
    /// 千分位换算小数；办公室/会客室为 pct
    pub total_efficiency: f64,
    pub final_efficiency: f64,
    pub trade_output_per_day: f64,
    pub manufacture_output_per_day: f64,
    // breakdown
    pub occupancy: f64,
    pub skill: f64,
    pub global: f64,
    pub order_multiplier: f64,
    pub trade_equivalent_efficiency: f64,
    pub gold_equivalent_efficiency: f64,
    pub combined_output_value: f64,
}

impl PartialEvaluator {
    pub fn new(layout_path: &str, box_path: &str) -> Result<Self, String> {
        let blueprint = NormalizedBlueprint::from_path(layout_path)
            .map_err(|e| format!("layout {layout_path}: {e:?}"))?;
        let box_text =
            std::fs::read_to_string(box_path).map_err(|e| format!("read box {box_path}: {e}"))?;
        let box_json: serde_json::Value =
            serde_json::from_str(&box_text).map_err(|e| format!("parse box: {e}"))?;
        let box_state = BoxState::from_json_with_mode(&box_text, BoxInputMode::FileCompatible)
            .map_err(|e| format!("box state: {e:?}"))?;
        let entries: Vec<crate::inputs::BoxOperator> =
            serde_json::from_str(&box_text).map_err(|e| format!("parse box entries: {e}"))?;
        let box_identity = entries
            .into_iter()
            .map(|op| (op.name, (op.elite, op.level, op.rarity, op.own)))
            .collect();
        Ok(Self {
            blueprint,
            box_json,
            box_state,
            box_identity,
        })
    }

    /// 房间容量（按布局房间定义，取不到时按设施类型默认）
    pub fn room_capacity(&self, room_id: &str) -> usize {
        if let Some(index) = self.blueprint.room_ids.iter().position(|id| id == room_id) {
            let room = self.blueprint.base.rooms[index];
            return room.capacity.max(1) as usize;
        }
        match room_kind(room_id) {
            "control" => 5,
            "meeting" => 2,
            "dorm" => 5,
            "power" | "office" | "workshop" | "training" => 1,
            _ => 3,
        }
    }

    /// 构造覆盖后的 operbox 文本（只提升精英化档，不把组合锚点当作满级要求）
    fn synthesize_box_text(&self, overrides: &HashMap<String, u8>) -> String {
        let mut value = self.box_json.clone();
        if let Some(list) = value.as_array_mut() {
            for op in list.iter_mut() {
                let Some(name) = op.get("name").and_then(|v| v.as_str()) else {
                    continue;
                };
                if let Some(elite) = overrides.get(name) {
                    let current = op.get("elite").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                    if *elite > current {
                        op["elite"] = serde_json::json!(elite);
                        op["own"] = serde_json::json!(true);
                    }
                }
            }
        }
        serde_json::to_string(&value).unwrap_or_default()
    }

    /// 对一组房间规格做部分评估。`overrides` 汇总所有房间的锚度覆盖。
    pub fn evaluate(&self, rooms: &[RoomSpec], shift_hours: u32) -> Result<PartialResult, String> {
        let mut all_overrides: HashMap<String, u8> = HashMap::new();
        for room in rooms {
            all_overrides.extend(room.overrides.clone());
        }
        let box_text = self.synthesize_box_text(&all_overrides);
        let owned_state: Option<BoxState> = if all_overrides.is_empty() {
            None
        } else {
            Some(
                BoxState::from_json_with_mode(&box_text, BoxInputMode::FileCompatible)
                    .map_err(|e| format!("synth box: {e:?}"))?,
            )
        };
        let box_state = owned_state.as_ref().unwrap_or(&self.box_state);

        let mut room_ids = HashSet::new();
        let mut assigned_names = HashSet::new();
        let mut assignment_rooms: Vec<V2RoomAssignment> = Vec::new();
        for room in rooms {
            if !room_ids.insert(room.room_id.clone()) {
                return Err(format!("room {} appears more than once", room.room_id));
            }
            let capacity = self.room_capacity(&room.room_id);
            if room.operators.len() > capacity {
                return Err(format!(
                    "room {} has {} operators but capacity is {}",
                    room.room_id,
                    room.operators.len(),
                    capacity
                ));
            }
            let mut operators: Vec<V2AssignedOperator> = Vec::new();
            for name in &room.operators {
                if !assigned_names.insert(name.clone()) {
                    return Err(format!("operator {name} is assigned more than once"));
                }
                let (base_elite, base_level, rarity, own) = self
                    .box_identity
                    .get(name)
                    .map(|(e, l, r, own)| (*e, *l, *r, *own))
                    .ok_or_else(|| format!("operator {name} 不在干员盒中"))?;
                if !own {
                    return Err(format!("operator {name} 未拥有"));
                }
                // Override 只表达知识库要求的更高精英化档位；等级始终沿用 box。
                // 因此一、二星的 E0/Lv30 技能仍能由 v3 自己判断是否解锁。
                let (elite, level) = match room.overrides.get(name) {
                    Some(e) if *e > base_elite => (*e, base_level),
                    _ => (base_elite, base_level),
                };
                operators.push(V2AssignedOperator {
                    name: name.clone(),
                    elite,
                    level,
                    rarity,
                    work_mood: Some(24),
                });
            }
            assignment_rooms.push(V2RoomAssignment {
                room_id: room.room_id.clone(),
                operators,
            });
        }
        let assignment = V2Assignment {
            rooms: assignment_rooms,
            training_assist: None,
            base_workforce: Vec::new(),
        };

        let summary =
            eval_v2_assignment_with_shift(&self.blueprint, box_state, &assignment, shift_hours)
                .map_err(|e| format!("eval: {e:?}"))?;

        let wanted: Vec<&str> = rooms.iter().map(|r| r.room_id.as_str()).collect();
        let lines = summary
            .rooms
            .iter()
            .filter(|room| wanted.contains(&room.room_id.as_str()))
            .map(|room| {
                let is_pct = matches!(
                    room.kind,
                    arknights_infra_v3::facility_state::FacilityKind::Office
                        | arknights_infra_v3::facility_state::FacilityKind::Meeting
                );
                let scale = |v: i32| if is_pct { v as f64 } else { v as f64 / 1000.0 };
                let (occ, skill, global, mult, teq, geq, cov) = room
                    .breakdown
                    .map(|b| {
                        (
                            b.occupancy as f64 / 1000.0,
                            b.skill as f64 / 1000.0,
                            b.global as f64 / 1000.0,
                            b.order_multiplier as f64 / 1000.0,
                            b.trade_equivalent_efficiency as f64 / 1000.0,
                            b.gold_equivalent_efficiency as f64 / 1000.0,
                            b.combined_output_value as f64 / 1000.0,
                        )
                    })
                    .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
                PartialRoomLine {
                    room_id: room.room_id.clone(),
                    kind: format!("{:?}", room.kind),
                    level: room.level,
                    total_efficiency: scale(room.total_efficiency),
                    final_efficiency: scale(room.final_efficiency),
                    trade_output_per_day: room.trade_output_per_day,
                    manufacture_output_per_day: room.manufacture_output_per_day,
                    occupancy: occ,
                    skill,
                    global,
                    order_multiplier: mult,
                    trade_equivalent_efficiency: teq,
                    gold_equivalent_efficiency: geq,
                    combined_output_value: cov,
                }
            })
            .collect();
        Ok(PartialResult { rooms: lines })
    }

    /// 评估单房 + 指定干员（便捷入口，锚度按需覆盖）
    pub fn evaluate_room(
        &self,
        room_id: &str,
        operators: &[(&str, Option<u8>)],
        shift_hours: u32,
    ) -> Result<PartialResult, String> {
        let mut overrides = HashMap::new();
        let mut names = Vec::new();
        for (name, over) in operators {
            names.push(name.to_string());
            if let Some(e) = over {
                overrides.insert(name.to_string(), *e);
            }
        }
        self.evaluate(
            &[RoomSpec {
                room_id: room_id.to_string(),
                operators: names,
                overrides,
            }],
            shift_hours,
        )
    }
}
