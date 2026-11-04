//! 部分求解评估器：把「部分干员 / 组合 / 体系」构造成固定作业，调用
//! ArknightsInfraCalc-v2 的效率评估函数结算，并支持锚度覆盖与心情注入。
//!
//! 改造点（相对 v2 的 `layout eval` CLI 全量口径）：
//! 1. 只提交目标房间——其余房间留空，实现子集求解；
//! 2. 锚度覆盖：组合成员可按知识库锚点练度评估（合成 operbox 提升精英化档），
//!    解决「box 练度低于组合档位时无法核对该档效率」的问题；
//! 3. `work_mood` 注入：默认满心情 24 评估（与知识库锚点口径一致）。

use crate::inputs::room_kind;
use infra_core::instances::{default_instances_path, OperatorInstances};
use infra_core::layout::{AssignedOperator, BaseAssignment, BaseBlueprint, RoomId};
use infra_core::skill_table::default_skill_table_path;
use infra_core::{evaluate_base_assignment_efficiencies, OperBox, SkillTable};
use std::collections::{HashMap, HashSet};

pub struct PartialEvaluator {
    pub blueprint: BaseBlueprint,
    pub box_json: serde_json::Value,
    pub box_state: OperBox,
    pub instances: OperatorInstances,
    pub skill_table: SkillTable,
    /// 冷路径的姓名 → 身份快照；结算热路径仍由 v2 的整数状态负责。
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
    /// 千分位换算小数
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
    pub fn kind_of(&self, room_id: &str) -> Result<infra_core::layout::FacilityKind, String> {
        self.blueprint
            .room(&RoomId::new(room_id))
            .map(|room| room.kind)
            .ok_or_else(|| format!("unknown room {room_id}"))
    }

    pub fn new(layout_path: &str, box_path: &str) -> Result<Self, String> {
        let blueprint = BaseBlueprint::load(std::path::Path::new(layout_path))
            .map_err(|e| format!("layout {layout_path}: {e:?}"))?;
        let box_text =
            std::fs::read_to_string(box_path).map_err(|e| format!("read box {box_path}: {e}"))?;
        let box_json: serde_json::Value =
            serde_json::from_str(&box_text).map_err(|e| format!("parse box: {e}"))?;
        let box_state = OperBox::from_json(&box_text).map_err(|e| format!("box state: {e:?}"))?;
        let instances = OperatorInstances::load(
            &default_instances_path().map_err(|e| format!("instances path: {e:?}"))?,
        )
        .map_err(|e| format!("instances: {e:?}"))?;
        let skill_table = SkillTable::load(
            &default_skill_table_path().map_err(|e| format!("skill table path: {e:?}"))?,
        )
        .map_err(|e| format!("skill table: {e:?}"))?;
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
            instances,
            skill_table,
            box_identity,
        })
    }

    /// 房间容量（按布局房间定义，取不到时按设施类型默认）
    pub fn room_capacity(&self, room_id: &str) -> usize {
        if let Some(room) = self.blueprint.room(&RoomId::new(room_id)) {
            return room.operator_capacity();
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
        let owned_state: Option<OperBox> = if all_overrides.is_empty() {
            None
        } else {
            Some(OperBox::from_json(&box_text).map_err(|e| format!("synth box: {e:?}"))?)
        };
        let box_state = owned_state.as_ref().unwrap_or(&self.box_state);

        let mut room_ids = HashSet::new();
        let mut assigned_names = HashSet::new();
        let mut assignment = BaseAssignment::default();
        for room in rooms {
            if !room_ids.insert(room.room_id.clone()) {
                return Err(format!("room {} appears more than once", room.room_id));
            }
            let blueprint_room = self
                .blueprint
                .room(&RoomId::new(&room.room_id))
                .ok_or_else(|| format!("unknown room {}", room.room_id))?;
            let capacity = blueprint_room.operator_capacity();
            if room.operators.len() > capacity {
                return Err(format!(
                    "room {} has {} operators but capacity is {}",
                    room.room_id,
                    room.operators.len(),
                    capacity
                ));
            }
            let mut operators: Vec<AssignedOperator> = Vec::new();
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
                // 因此一、二星的 E0/Lv30 技能仍能由 v2 自己判断是否解锁。
                let (elite, level) = match room.overrides.get(name) {
                    Some(e) if *e > base_elite => (*e, base_level),
                    _ => (base_elite, base_level),
                };
                operators.push(AssignedOperator {
                    name: name.clone(),
                    elite,
                    level,
                    rarity,
                    work_mood: Some(24),
                });
            }
            assignment.set_room(room.room_id.as_str(), operators);
        }
        self.evaluate_assignment(&assignment, f64::from(shift_hours), box_state, Some(rooms))
    }

    /// 排班复评直接重放公共作业，保留宿舍、练度、心情和小数班长。
    pub fn replay_shift(&self, shift: &crate::inputs::Shift) -> Result<PartialResult, String> {
        let assignment: BaseAssignment = serde_json::from_value(
            serde_json::to_value(&shift.assignment).map_err(|e| format!("assignment: {e}"))?,
        )
        .map_err(|e| format!("assignment: {e}"))?;
        self.evaluate_assignment(&assignment, shift.duration_hours, &self.box_state, None)
    }

    fn evaluate_assignment(
        &self,
        assignment: &BaseAssignment,
        shift_hours: f64,
        box_state: &OperBox,
        requested_rooms: Option<&[RoomSpec]>,
    ) -> Result<PartialResult, String> {
        let summary = evaluate_base_assignment_efficiencies(
            &self.blueprint,
            assignment,
            &self.instances,
            &self.skill_table,
            shift_hours,
            Some(box_state.durin_dorm_planning_count(&self.instances)),
        )
        .map_err(|e| format!("eval: {e:?}"))?;

        let wanted: Option<Vec<&str>> =
            requested_rooms.map(|rooms| rooms.iter().map(|r| r.room_id.as_str()).collect());
        let lines = summary
            .room_lines
            .iter()
            .filter(|room| {
                wanted
                    .as_ref()
                    .is_none_or(|wanted| wanted.contains(&room.room_id.as_str()))
            })
            .map(|room| {
                let blueprint_room = self
                    .blueprint
                    .room(&RoomId::new(&room.room_id))
                    .expect("settled room must exist in blueprint");
                PartialRoomLine {
                    room_id: room.room_id.clone(),
                    kind: format!("{:?}", blueprint_room.kind),
                    level: blueprint_room.level,
                    total_efficiency: room.total_efficiency.as_f64(),
                    final_efficiency: room.final_efficiency.as_f64(),
                    trade_output_per_day: room.trade_output_per_day,
                    manufacture_output_per_day: room.manufacture_output_per_day,
                    occupancy: if blueprint_room.kind == infra_core::layout::FacilityKind::Recycle {
                        room.operator_count as f64 * 0.01
                    } else {
                        (room.station_efficiency - room.global_efficiency).as_f64()
                    },
                    skill: if blueprint_room.kind == infra_core::layout::FacilityKind::PowerPlant {
                        room.power_skill_efficiency.as_f64()
                    } else {
                        room.operator_efficiency.as_f64()
                    },
                    global: room.global_efficiency.as_f64(),
                    order_multiplier: room.order_multiplier.as_f64(),
                    trade_equivalent_efficiency: room.trade_equivalent_efficiency.as_f64(),
                    gold_equivalent_efficiency: room.gold_equivalent_efficiency.as_f64(),
                    combined_output_value: room.combined_output_value.as_f64(),
                }
            })
            .collect::<Vec<_>>();
        for room in requested_rooms.unwrap_or(&[]).iter().filter(|room| {
            matches!(
                self.kind_of(&room.room_id),
                Ok(infra_core::layout::FacilityKind::TradePost
                    | infra_core::layout::FacilityKind::Factory
                    | infra_core::layout::FacilityKind::PowerPlant
                    | infra_core::layout::FacilityKind::Recycle)
            )
        }) {
            if !lines.iter().any(|line| line.room_id == room.room_id) {
                return Err(format!("room {} has no v2 efficiency result", room.room_id));
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_trade_equivalents_are_replayed_from_core() {
        let layout = infra_core::default_layout_243_path().unwrap();
        let box_path = infra_core::default_operbox_full_e2_path().unwrap();
        let evaluator =
            PartialEvaluator::new(layout.to_str().unwrap(), box_path.to_str().unwrap()).unwrap();
        let result = evaluator
            .evaluate_room(
                "trade_1",
                &[("巫恋", None), ("龙舌兰", None), ("卡夫卡", None)],
                24,
            )
            .unwrap();
        let line = &result.rooms[0];
        assert_eq!(line.kind, "TradePost");
        assert_eq!(line.skill, 0.9);
        assert_eq!(line.trade_equivalent_efficiency, 1.365);
        assert_eq!(line.gold_equivalent_efficiency, 0.45);
        assert_eq!(line.combined_output_value, 2.845);
        assert!(evaluator
            .evaluate_room("trade_unknown", &[("巫恋", None)], 24)
            .is_err());
    }
}
