//! 输入解析：干员盒（operbox）与 plan.compute 结果 JSON（schema v1–4 容忍读）。

use serde::Deserialize;
use std::collections::HashMap;

// ---- 干员盒 ----

#[derive(Debug, Clone, Deserialize)]
pub struct BoxOperator {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub elite: u8,
    #[serde(default)]
    pub level: u32,
    #[serde(default = "default_true")]
    pub own: bool,
    #[serde(default)]
    pub potential: u8,
    #[serde(default)]
    pub rarity: u8,
}

fn default_true() -> bool {
    true
}

pub struct OperBox {
    pub operators: Vec<BoxOperator>,
    pub by_name: HashMap<String, usize>,
}

impl OperBox {
    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("read box {path}: {e}"))?;
        let operators: Vec<BoxOperator> =
            serde_json::from_str(&text).map_err(|e| format!("parse box {path}: {e}"))?;
        let by_name = operators
            .iter()
            .enumerate()
            .map(|(i, op)| (op.name.clone(), i))
            .collect();
        Ok(Self { operators, by_name })
    }

    pub fn get(&self, name: &str) -> Option<&BoxOperator> {
        self.by_name.get(name).map(|&i| &self.operators[i])
    }

    pub fn owned(&self, name: &str) -> bool {
        self.get(name).map(|op| op.own).unwrap_or(false)
    }
}

// ---- plan.compute 结果 ----

#[derive(Debug, Deserialize)]
pub struct PlanEnvelope {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub result: Option<PlanResult>,
    #[serde(default)]
    pub error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct PlanResult {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub rotation: Rotation,
    #[serde(default)]
    pub maa: serde_json::Value,
    #[serde(default)]
    pub profile: serde_json::Value,
    #[serde(default)]
    pub training_advice: TrainingAdvice,
}

#[derive(Debug, Default, Deserialize)]
pub struct Rotation {
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub daily: DailySummary,
    #[serde(default)]
    pub shifts: Vec<Shift>,
}

#[derive(Debug, Default, Deserialize)]
pub struct DailySummary {
    #[serde(default)]
    pub trade: f64,
    #[serde(default)]
    pub manufacture: f64,
    #[serde(default)]
    pub power: f64,
    #[serde(default)]
    pub production: Production,
}

#[derive(Debug, Default, Deserialize)]
pub struct Production {
    #[serde(default)]
    pub lmd: f64,
    #[serde(default)]
    pub pure_gold: f64,
    #[serde(default)]
    pub battle_records: f64,
    #[serde(default)]
    pub orundum: f64,
    #[serde(default)]
    pub originium_shards: f64,
}

#[derive(Debug, Default, Deserialize)]
pub struct Shift {
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub duration_hours: f64,
    #[serde(default)]
    pub active_teams: Vec<String>,
    #[serde(default)]
    pub resting_team: String,
    #[serde(default)]
    pub assignment: ShiftAssignment,
    #[serde(default)]
    pub efficiencies: ShiftEfficiencies,
    #[serde(default)]
    pub weighted_trade: f64,
    #[serde(default)]
    pub weighted_manufacture: f64,
    #[serde(default)]
    pub weighted_power: f64,
}

#[derive(Debug, Default, Deserialize)]
pub struct ShiftAssignment {
    #[serde(default)]
    pub rooms: Vec<ShiftRoom>,
    #[serde(default)]
    pub training_assist: Option<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ShiftRoom {
    pub room_id: String,
    #[serde(default)]
    pub operators: Vec<ShiftOperator>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ShiftOperator {
    pub name: String,
    #[serde(default)]
    pub elite: u8,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub rarity: u8,
}

#[derive(Debug, Default, Deserialize)]
pub struct ShiftEfficiencies {
    #[serde(default)]
    pub trade_efficiency: f64,
    #[serde(default)]
    pub manufacture_efficiency: f64,
    #[serde(default)]
    pub power_efficiency: f64,
    #[serde(default)]
    pub room_lines: Vec<RoomLine>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RoomLine {
    pub room_id: String,
    #[serde(default)]
    pub base_efficiency: f64,
    #[serde(default)]
    pub total_efficiency: f64,
    #[serde(default)]
    pub operator_efficiency: f64,
    #[serde(default)]
    pub operator_count: i32,
    #[serde(default)]
    pub global_efficiency: f64,
    #[serde(default)]
    pub station_efficiency: f64,
    #[serde(default)]
    pub trade_efficiency: f64,
    #[serde(default)]
    pub trade_equivalent_efficiency: f64,
    #[serde(default)]
    pub gold_equivalent_efficiency: f64,
    #[serde(default)]
    pub trade_output_per_day: f64,
    #[serde(default)]
    pub order_multiplier: f64,
    #[serde(default)]
    pub combined_output_value: f64,
    #[serde(default)]
    pub manufacture_efficiency: f64,
    #[serde(default)]
    pub manufacture_output_per_day: f64,
    #[serde(default)]
    pub manufacture_unit_output_per_day: f64,
    #[serde(default)]
    pub manufacture_skill_efficiency: f64,
    #[serde(default)]
    pub power_efficiency: f64,
    #[serde(default)]
    pub power_skill_efficiency: f64,
    #[serde(default)]
    pub equivalent_efficiency: f64,
}

#[derive(Debug, Default, Deserialize)]
pub struct TrainingAdvice {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub context: serde_json::Value,
    #[serde(default)]
    pub combinations: Vec<TrainingCombination>,
    #[serde(default)]
    pub recommendations: Vec<serde_json::Value>,
    #[serde(default)]
    pub newbie_section_status: String,
    #[serde(default)]
    pub incomplete_newbie: Vec<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct TrainingCombination {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub completed_slots: i32,
    #[serde(default)]
    pub completion_percent: i32,
    #[serde(default)]
    pub members: Vec<TrainingMember>,
}

#[derive(Debug, Default, Deserialize)]
pub struct TrainingMember {
    pub operator: String,
    #[serde(default)]
    pub owned: bool,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub progress: String,
}

impl PlanEnvelope {
    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("read plan {path}: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("parse plan {path}: {e}"))
    }
}

/// MAA 导出块的菲亚梅塔目标（班次序 → target）
pub fn fiammetta_targets(result: &PlanResult) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let Some(plans) = result.maa.get("plans").and_then(|v| v.as_array()) else {
        return out;
    };
    for (i, plan) in plans.iter().enumerate() {
        if let Some(target) = plan
            .get("Fiammetta")
            .and_then(|f| f.get("target"))
            .and_then(|t| t.as_str())
        {
            if !target.is_empty() {
                out.push((i, target.to_string()));
            }
        }
    }
    out
}

/// MAA 导出块的无人机目标（班次序 → (room, index)）
pub fn drone_targets(result: &PlanResult) -> Vec<(usize, String, i64)> {
    let mut out = Vec::new();
    let Some(plans) = result.maa.get("plans").and_then(|v| v.as_array()) else {
        return out;
    };
    for (i, plan) in plans.iter().enumerate() {
        if let Some(d) = plan.get("drones") {
            let enabled = d.get("enable").and_then(|v| v.as_bool()).unwrap_or(false);
            if enabled {
                let room = d
                    .get("room")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let index = d.get("index").and_then(|v| v.as_i64()).unwrap_or(1);
                out.push((i, room, index));
            }
        }
    }
    out
}

/// 从 rotation 房间 id 推设施类型（trade_1 → trade 等）
pub fn room_kind(room_id: &str) -> &'static str {
    if room_id.starts_with("trade") {
        "trade"
    } else if room_id.starts_with("manu") {
        "manufacture"
    } else if room_id.starts_with("power") {
        "power"
    } else if room_id == "control" {
        "control"
    } else if room_id.starts_with("dorm") {
        "dorm"
    } else if room_id == "meeting" {
        "meeting"
    } else if room_id.starts_with("office") {
        "office"
    } else if room_id == "workshop" {
        "workshop"
    } else if room_id == "training" {
        "training"
    } else {
        "other"
    }
}
