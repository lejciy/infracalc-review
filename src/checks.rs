//! skill-9 排班表审查的十四项检查（机械化部分）。
//! 输入：plan.compute 结果 + operbox + 布局房间表 + 组合登记表。

use crate::inputs::*;
use crate::kb::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, serde::Serialize)]
pub struct Finding {
    pub check: u8,
    pub severity: String, // "error" | "improvement" | "note"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shift: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,
    pub subject: String,
    pub message: String,
    pub kb_ref: String,
}

fn error(
    check: u8,
    shift: Option<usize>,
    room: Option<String>,
    subject: &str,
    msg: &str,
    kb: &str,
) -> Finding {
    Finding {
        check,
        severity: "error".into(),
        shift,
        room,
        subject: subject.into(),
        message: msg.into(),
        kb_ref: kb.into(),
    }
}

fn improve(
    check: u8,
    shift: Option<usize>,
    room: Option<String>,
    subject: &str,
    msg: &str,
    kb: &str,
) -> Finding {
    Finding {
        check,
        severity: "improvement".into(),
        shift,
        room,
        subject: subject.into(),
        message: msg.into(),
        kb_ref: kb.into(),
    }
}

fn note(
    check: u8,
    shift: Option<usize>,
    room: Option<String>,
    subject: &str,
    msg: &str,
    kb: &str,
) -> Finding {
    Finding {
        check,
        severity: "note".into(),
        shift,
        room,
        subject: subject.into(),
        message: msg.into(),
        kb_ref: kb.into(),
    }
}

/// 协议层问题不属于十四项机制检查，但必须进入统一报告，避免命令只打印
/// 解析错误而丢失 `plan.compute` 的失败原因。
pub fn protocol_finding(message: &str) -> Finding {
    error(
        0,
        None,
        None,
        "plan.compute",
        message,
        "serve 协议·plan.compute 失败响应",
    )
}

/// 布局房间静态信息（从布局 JSON 简单解析）
pub struct LayoutInfo {
    pub rooms: HashMap<String, (String, u8, String)>, // room_id → (kind, level, product)
    pub capacities: HashMap<String, usize>,
    pub dorm_levels: Vec<u8>,
    pub template: String,
}

impl LayoutInfo {
    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("read layout: {e}"))?;
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("parse layout: {e}"))?;
        let mut rooms = HashMap::new();
        let mut capacities = HashMap::new();
        let mut dorm_levels = Vec::new();
        let mut template = String::new();
        if let Some(t) = v.get("template").and_then(|t| t.as_str()) {
            template = t.to_string();
        }
        if let Some(list) = v.get("rooms").and_then(|r| r.as_array()) {
            for room in list {
                let id = room
                    .get("id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let kind = room
                    .get("kind")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let level = room.get("level").and_then(|x| x.as_u64()).unwrap_or(1) as u8;
                let dorm_beds = room
                    .get("dorm_beds")
                    .and_then(|x| x.as_u64())
                    .map(|x| x as usize);
                let product = room
                    .get("product")
                    .map(|p| p.to_string().replace(['"', '{', '}', ' '], ""))
                    .unwrap_or_default();
                if kind.contains("dorm") {
                    dorm_levels.push(level);
                }
                let capacity = match kind.as_str() {
                    "control_center" | "control" => level.clamp(1, 5) as usize,
                    "trade_post" | "factory" | "trade" | "manufacture" => {
                        level.clamp(1, 3) as usize
                    }
                    "meeting_room" | "meeting" => 2,
                    "dormitory" | "dorm" => dorm_beds.unwrap_or(5),
                    "power_plant" | "power" | "office" | "training_room" | "training"
                    | "workshop" => 1,
                    _ => 1,
                };
                rooms.insert(id, (kind, level, product));
                capacities.insert(
                    room.get("id")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    capacity,
                );
            }
        }
        Ok(Self {
            rooms,
            capacities,
            dorm_levels,
            template,
        })
    }

    pub fn level_of(&self, room_id: &str) -> u8 {
        self.rooms.get(room_id).map(|(_, l, _)| *l).unwrap_or(3)
    }

    pub fn kind_of(&self, room_id: &str) -> String {
        self.rooms
            .get(room_id)
            .map(|(k, _, _)| k.clone())
            .unwrap_or_else(|| room_kind(room_id).to_string())
    }

    /// 与 v3 `NormalizedBlueprint::capacity_for` 对齐的冷路径容量推导。
    pub fn capacity_of(&self, room_id: &str) -> usize {
        if let Some(capacity) = self.capacities.get(room_id) {
            return *capacity;
        }
        let Some((kind, level, _)) = self.rooms.get(room_id) else {
            return match room_kind(room_id) {
                "control" => 5,
                "trade" | "manufacture" => 3,
                "meeting" => 2,
                "dorm" => 5,
                _ => 1,
            };
        };
        match kind.as_str() {
            "control_center" | "control" => (*level).clamp(1, 5) as usize,
            "trade_post" | "factory" | "trade" | "manufacture" => (*level).clamp(1, 3) as usize,
            "meeting_room" | "meeting" => 2,
            "dormitory" | "dorm" => 5,
            "power_plant" | "power" | "office" | "training_room" | "training" | "workshop" => 1,
            _ => 1,
        }
    }

    /// 制造配方（gold / battle_record / originium）
    pub fn manu_recipe(&self, room_id: &str) -> String {
        self.rooms
            .get(room_id)
            .map(|(_, _, p)| match p.as_str() {
                x if x.contains("gold") && !x.contains("battle") => "gold".into(),
                x if x.contains("battle") => "battle_record".into(),
                x if x.contains("originium") => "originium".into(),
                _ => String::new(),
            })
            .unwrap_or_default()
    }

    /// 贸易订单（gold=LMD / originium=开采协力）
    pub fn trade_order(&self, room_id: &str) -> String {
        self.rooms
            .get(room_id)
            .map(|(_, _, p)| {
                if p.contains("originium") {
                    "originium".into()
                } else {
                    "gold".into()
                }
            })
            .unwrap_or_else(|| "gold".into())
    }

    fn min_dorm_recovery(&self) -> f64 {
        if self.dorm_levels.is_empty() {
            dorm_recovery(1)
        } else {
            dorm_recovery(*self.dorm_levels.iter().min().unwrap())
        }
    }
}

/// 每班「在岗」视图：干员 → 房间（宿舍以外的进驻；宿舍按需单列）
struct ShiftView<'a> {
    duty: HashMap<String, String>, // name → room_id（不含宿舍）
    dorm: HashMap<String, String>, // name → dorm room_id
    rooms: &'a [ShiftRoom],
}

fn shift_view(shift: &Shift) -> ShiftView<'_> {
    let mut duty = HashMap::new();
    let mut dorm = HashMap::new();
    for room in &shift.assignment.rooms {
        for op in &room.operators {
            if room.room_id.starts_with("dorm") {
                dorm.insert(op.name.clone(), room.room_id.clone());
            } else {
                duty.insert(op.name.clone(), room.room_id.clone());
            }
        }
    }
    ShiftView {
        duty,
        dorm,
        rooms: &shift.assignment.rooms,
    }
}

pub fn run_all_checks(
    result: &PlanResult,
    box_data: &OperBox,
    layout: &LayoutInfo,
    registry: &ComboRegistry,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let shifts = &result.rotation.shifts;
    let views: Vec<ShiftView> = shifts.iter().map(shift_view).collect();
    let fiam_targets = fiammetta_targets(result);
    let drone_targets = drone_targets(result);

    check0_assignment_integrity(shifts, box_data, layout, &mut findings);
    check1_aura(shifts, &views, &mut findings);
    check2_combo_shift(registry, shifts, &views, &mut findings);
    check3_trade(result, layout, box_data, &views, &mut findings);
    check4_manu(layout, &views, &mut findings);
    check5_meeting(&views, &mut findings);
    check6_multi_use(box_data, &views, &mut findings);
    check7_mood(shifts, &views, layout, &mut findings);
    check8_continuous(shifts, &views, &fiam_targets, &mut findings);
    check9_pendants(shifts, &views, &mut findings);
    check10_recalc(result, layout, &mut findings);
    check11_drones(result, &drone_targets, &mut findings);
    check12_charm(&views, &mut findings);
    check13_product(layout, &views, &mut findings);
    check14_office_meeting(box_data, &views, registry, &mut findings);
    check15_result_metadata(result, registry, &mut findings);
    findings
}

// ---- 检查 0：Assignment 结构完整性 ----

/// 其它检查使用姓名索引以便生成可读报告；在建立索引前先拒绝重复上岗，
/// 避免 HashMap 覆盖重复成员后把结构错误伪装成正常排班。
fn check0_assignment_integrity(
    shifts: &[Shift],
    box_data: &OperBox,
    layout: &LayoutInfo,
    out: &mut Vec<Finding>,
) {
    for (si, shift) in shifts.iter().enumerate() {
        let mut seen: HashMap<&str, &str> = HashMap::new();
        let mut room_ids = HashSet::new();
        for room in &shift.assignment.rooms {
            if !room_ids.insert(room.room_id.as_str()) {
                out.push(error(
                    0,
                    Some(si),
                    Some(room.room_id.clone()),
                    "duplicate_room",
                    "同一班次重复声明同一个房间",
                    "求解器架构与状态设计·Assignment 房间唯一性",
                ));
            }
            let capacity = layout.capacity_of(&room.room_id);
            if room.operators.len() > capacity {
                out.push(error(
                    0,
                    Some(si),
                    Some(room.room_id.clone()),
                    "room_capacity",
                    &format!(
                        "房间有 {} 名干员，超过审查器推断容量 {}",
                        room.operators.len(),
                        capacity
                    ),
                    "求解器架构与状态设计·Assignment 容量约束",
                ));
            }
            if !layout.rooms.contains_key(&room.room_id) {
                out.push(error(
                    0,
                    Some(si),
                    Some(room.room_id.clone()),
                    "unknown_room",
                    "排班引用了布局中不存在的房间",
                    "体系组合与计划执行·计划落位失败语义",
                ));
            }
            for op in &room.operators {
                if let Some(box_op) = box_data.get(&op.name) {
                    if !box_op.own {
                        out.push(error(
                            0,
                            Some(si),
                            Some(room.room_id.clone()),
                            &op.name,
                            "排班引用了干员盒中标记为未拥有的干员",
                            "输入契约·operbox own",
                        ));
                    }
                } else {
                    out.push(error(
                        0,
                        Some(si),
                        Some(room.room_id.clone()),
                        &op.name,
                        "排班引用了干员盒中不存在的干员",
                        "输入契约·operbox operator name",
                    ));
                }
                if let Some(previous) = seen.insert(op.name.as_str(), room.room_id.as_str()) {
                    out.push(error(
                        0,
                        Some(si),
                        None,
                        op.name.as_str(),
                        &format!("同一班次重复上岗：{} 与 {}", previous, room.room_id),
                        "求解器架构与状态设计·Assignment 唯一位置",
                    ));
                }
            }
        }
    }
}

// ---- 检查 1：中枢同类光环重复 ----

fn check1_aura(_shifts: &[Shift], views: &[ShiftView], out: &mut Vec<Finding>) {
    for (si, view) in views.iter().enumerate() {
        let control: Vec<&String> = view
            .duty
            .iter()
            .filter(|(_, r)| r.as_str() == "control")
            .map(|(n, _)| n)
            .collect();
        if control.is_empty() {
            continue;
        }
        for family in AURA_FAMILIES {
            let hits: Vec<&&String> = control
                .iter()
                .filter(|n| family.members.contains(&n.as_str()))
                .collect();
            if hits.len() >= 2 {
                let names: Vec<String> = hits.iter().map(|n| n.to_string()).collect();
                out.push(improve(
                    1,
                    Some(si),
                    Some("control".into()),
                    &names.join("、"),
                    &format!(
                        "中枢存在同种光环家族 {} 的多名干员，同种效果取最高、其余纯占位",
                        family.family
                    ),
                    family.kb_ref,
                ));
            }
        }
        let mood_hits: Vec<&&String> = control
            .iter()
            .filter(|n| MOOD_RECOVERY_NAMED.contains(&n.as_str()))
            .collect();
        if mood_hits.len() >= 2 {
            out.push(note(
                1,
                Some(si),
                Some("control".into()),
                &format!("{}、{}", mood_hits[0], mood_hits[1]),
                "公事公办/孤光共照/巴别塔之帜互不叠加、取最高生效，同时进驻只有一份生效",
                "心情与宿舍·减免来源与叠加",
            ));
        }
        if control.len() < 5 {
            out.push(improve(
                1,
                Some(si),
                Some("control".into()),
                "中枢",
                &format!(
                    "中枢只进驻 {} 人，建议满 5 人拿满全局心情减免 −0.25/h",
                    control.len()
                ),
                "控制中枢机制·基础效果",
            ));
        }
    }
}

// ---- 检查 2：组合成员错班 ----

fn combo_required(m: &ComboMember) -> bool {
    (m.role.starts_with("core") || m.role.starts_with("key")) && m.role != "key_trigger"
}

fn member_room_allowed_in_dorm(m: &ComboMember) -> bool {
    m.role.contains("dorm")
}

fn check2_combo_shift(
    registry: &ComboRegistry,
    _shifts: &[Shift],
    views: &[ShiftView],
    out: &mut Vec<Finding>,
) {
    if views.is_empty() {
        return;
    }
    for combo in &registry.combos {
        let required: Vec<&ComboMember> =
            combo.members.iter().filter(|m| combo_required(m)).collect();
        if required.is_empty() {
            continue;
        }
        let mut any_full = false;
        let mut per_member: HashMap<&str, Vec<usize>> = HashMap::new();
        let mut per_member_room: HashMap<(&str, usize), String> = HashMap::new();
        for (si, view) in views.iter().enumerate() {
            let mut full = true;
            for m in &required {
                let room = if member_room_allowed_in_dorm(m) {
                    view.duty.get(&m.name).or_else(|| view.dorm.get(&m.name))
                } else {
                    view.duty.get(&m.name)
                };
                match room {
                    Some(r) => {
                        per_member.entry(m.name.as_str()).or_default().push(si);
                        per_member_room.insert((m.name.as_str(), si), r.clone());
                    }
                    None => full = false,
                }
            }
            if full {
                any_full = true;
            }
        }
        if any_full {
            // 单站组合再核同房：同班不同房 = 组合未同站启用
            if combo.layer.contains("单站") {
                for (si, _view) in views.iter().enumerate() {
                    let rooms: HashSet<String> = required
                        .iter()
                        .filter_map(|m| per_member_room.get(&(m.name.as_str(), si)).cloned())
                        .collect();
                    if rooms.len() > 1 && per_member.len() == required.len() {
                        out.push(error(
                            2,
                            Some(si),
                            None,
                            &combo.name,
                            &format!(
                                "单站组合成员同班但未同房（{:?}），组合无法在该站启用",
                                rooms.into_iter().collect::<Vec<_>>()
                            ),
                            "龙舌兰组·启用条件（三人进驻同一座三级贸易站）",
                        ));
                    }
                }
            }
            continue;
        }
        let active_members = per_member.keys().count();
        if active_members < 2 {
            continue; // 全员未启用或仅单人出现，缺员归 skill-3 口径
        }
        let never_placed: Vec<&str> = required
            .iter()
            .filter(|m| !per_member.contains_key(m.name.as_str()))
            .map(|m| m.name.as_str())
            .collect();
        let spread: Vec<String> = required
            .iter()
            .map(|m| {
                format!(
                    "{}→班{}",
                    m.name,
                    per_member
                        .get(m.name.as_str())
                        .map(|v| v
                            .iter()
                            .map(|i| (i + 1).to_string())
                            .collect::<Vec<_>>()
                            .join("/"))
                        .unwrap_or_else(|| "未上".into())
                )
            })
            .collect();
        if !never_placed.is_empty() {
            out.push(note(
                2,
                None,
                None,
                &combo.name,
                &format!(
                    "组合未满配启用（{} 未上场：{}）；在班成员按降级档运行",
                    never_placed.join("、"),
                    spread.join("，")
                ),
                "组合注册表·完成状态",
            ));
        } else {
            out.push(error(
                2,
                None,
                None,
                &combo.name,
                &format!(
                    "组合成员错班，无任何一个班次可以完整启用（{}）",
                    spread.join("，")
                ),
                "错班轮换·中枢的轮换（体系中枢跟随所属体系同上同下）",
            ));
        }
    }
}

// ---- 检查 3：贸易特殊订单互斥与巫恋龙舌兰 ----

fn check3_trade(
    _result: &PlanResult,
    layout: &LayoutInfo,
    box_data: &OperBox,
    views: &[ShiftView],
    out: &mut Vec<Finding>,
) {
    for (si, view) in views.iter().enumerate() {
        for room in view.rooms {
            let is_trade = layout.kind_of(&room.room_id).contains("trade");
            if !is_trade {
                continue;
            }
            let names: Vec<&str> = room.operators.iter().map(|o| o.name.as_str()).collect();
            // 同站互斥
            if names.contains(&"但书") && names.contains(&"可露希尔") {
                out.push(error(
                    3,
                    Some(si),
                    Some(room.room_id.clone()),
                    "但书×可露希尔",
                    "同站互斥：可露希尔特别订单优先级更高，抢走违约订单触发，但书技能空转",
                    "贸易站机制·特殊订单；但书·互斥与共存",
                ));
            }
            if names.contains(&"可露希尔") && names.contains(&"龙舌兰") {
                out.push(error(
                    3,
                    Some(si),
                    Some(room.room_id.clone()),
                    "可露希尔×龙舌兰",
                    "同站互斥：订单被改写为特别订单，龙舌兰投资失效",
                    "贸易站机制·特殊订单",
                ));
            }
            // 开采协力站内的改写干员（检查 13 亦覆盖，此处从互斥角度提示）
            if layout.trade_order(&room.room_id) == "originium" {
                for (n, _) in ORDER_REWRITERS {
                    if names.contains(n) {
                        out.push(error(
                            3,
                            Some(si),
                            Some(room.room_id.clone()),
                            *n,
                            "开采协力（源石订单）站没有赤金订单可改写，特殊订单技能失去作用对象",
                            "贸易站机制·谈判策略与特殊订单",
                        ));
                    }
                }
                if names
                    .iter()
                    .any(|n| TAILOR_BETA_HOLDERS.contains(n) || *n == "巫恋")
                {
                    out.push(error(
                        3,
                        Some(si),
                        Some(room.room_id.clone()),
                        "裁缝类",
                        "开采协力站没有赤金订单分布可提升，裁缝技能失去作用对象",
                        "可露希尔特别订单·边界与误区（同型口径）",
                    ));
                }
            }
            // 巫恋龙舌兰效率
            if names.contains(&"巫恋") {
                if let Some(op) = box_data.get("巫恋") {
                    if op.elite < 2 {
                        out.push(error(
                            3,
                            Some(si),
                            Some(room.room_id.clone()),
                            "巫恋",
                            "龙舌兰组启动刚需为巫恋精二（低语），当前练度不足",
                            "龙舌兰组·启用条件",
                        ));
                    }
                }
                if layout.level_of(&room.room_id) < 3 {
                    out.push(error(
                        3,
                        Some(si),
                        Some(room.room_id.clone()),
                        room.room_id.as_str(),
                        "龙舌兰组刚需三级贸易站",
                        "龙舌兰组·启用条件",
                    ));
                }
                let third: Vec<&&str> = names
                    .iter()
                    .filter(|n| **n != "巫恋" && **n != "龙舌兰")
                    .collect();
                if let Some(t) = third.first() {
                    let ok_tailor = TAILOR_BETA_HOLDERS.contains(t) || **t == "绮良";
                    if !ok_tailor {
                        out.push(improve(3, Some(si), Some(room.room_id.clone()), **t,
                            "巫恋组第三人应为裁缝 β 持有者或挂件（绮良）；纯效率散件被「低语」归零、白占位",
                            "龙舌兰组·边界与误区"));
                    }
                }
            }
        }
    }
}

// ---- 检查 4：制造归零互斥 ----

fn check4_manu(layout: &LayoutInfo, views: &[ShiftView], out: &mut Vec<Finding>) {
    for (si, view) in views.iter().enumerate() {
        for room in view.rooms {
            if !layout.kind_of(&room.room_id).contains("factory") {
                continue;
            }
            let names: Vec<&str> = room.operators.iter().map(|o| o.name.as_str()).collect();
            let zeroers: Vec<&&str> = names.iter().filter(|n| MANU_ZEROERS.contains(n)).collect();
            if zeroers.is_empty() {
                continue;
            }
            for n in &names {
                if MANU_ZERO_EXEMPT.contains(n) {
                    continue;
                }
                if ABYSSAL_HUNTERS.contains(n) {
                    out.push(error(
                        4,
                        Some(si),
                        Some(room.room_id.clone()),
                        *n,
                        "深海猎人与自动化组同站互斥：清零效果优先生效，集群狩猎加成被抹掉",
                        "组合总览·互斥规则",
                    ));
                } else {
                    out.push(error(
                        4,
                        Some(si),
                        Some(room.room_id.clone()),
                        *n,
                        "归零类技能同站：常规生产力被清零，该干员技能空转",
                        "自动化（释义）·易误读点",
                    ));
                }
            }
            // 槐琥与归零/设施数量类同站
            if names.contains(&"槐琥") {
                let readable: Vec<&&str> = names
                    .iter()
                    .filter(|n| !MANU_ZERO_EXEMPT.contains(n) && **n != "槐琥")
                    .collect();
                if readable.is_empty() || !zeroers.is_empty() {
                    out.push(error(4, Some(si), Some(room.room_id.clone()), "槐琥",
                        "槐琥配合意识读不到加成：清零类清空可读效率，清流/设施数量类不入读数——同站零转化纯占位",
                        "配合意识（释义）；深海猎人组·互斥与共存"));
                }
            }
        }
        // 深海×槐琥（无归零类时也互斥）
        for room in view.rooms {
            if !layout.kind_of(&room.room_id).contains("factory") {
                continue;
            }
            let names: Vec<&str> = room.operators.iter().map(|o| o.name.as_str()).collect();
            if names.contains(&"槐琥") && names.iter().any(|n| ABYSSAL_HUNTERS.contains(n)) {
                out.push(error(4, Some(si), Some(room.room_id.clone()), "槐琥×深海猎人",
                    "官方标注「集群狩猎」无法与「配合意识」叠加：中枢加成不入槐琥读数，深海猎人又无自身制造技能，槐琥零转化",
                    "组合总览·互斥规则"));
            }
        }
    }
}

// ---- 检查 5：会客室结构 ----

fn check5_meeting(views: &[ShiftView], out: &mut Vec<Finding>) {
    for (si, view) in views.iter().enumerate() {
        for room in view.rooms {
            if room.room_id != "meeting" {
                continue;
            }
            let names: Vec<&str> = room.operators.iter().map(|o| o.name.as_str()).collect();
            for (a, b) in MEETING_PARTNERS {
                if names.contains(a) && !names.contains(b) {
                    out.push(improve(
                        5,
                        Some(si),
                        Some("meeting".into()),
                        *a,
                        &format!("固定搭档缺位：{a} 需与 {b} 同室才拿满搭档加成"),
                        "会客室取向·固定双人搭档",
                    ));
                }
            }
        }
    }
}

// ---- 检查 6：多用法干员形态对照 ----

fn check6_multi_use(box_data: &OperBox, views: &[ShiftView], out: &mut Vec<Finding>) {
    for mu in MULTI_USE {
        for (si, view) in views.iter().enumerate() {
            let Some(room_id) = view.duty.get(mu.name) else {
                continue;
            };
            let room = view.rooms.iter().find(|r| &r.room_id == room_id);
            let Some(room) = room else { continue };
            let names: Vec<&str> = room.operators.iter().map(|o| o.name.as_str()).collect();
            let partners_present = mu
                .combo_members
                .iter()
                .filter(|m| **m != mu.name)
                .all(|m| names.contains(m));
            if partners_present {
                continue; // 已按组合形态使用
            }
            // 组合搭档是否可用（持有且本班有空）：持有即可作为改进建议
            let partners_owned: Vec<&str> = mu
                .combo_members
                .iter()
                .filter(|m| **m != mu.name && box_data.owned(*m))
                .copied()
                .collect();
            if partners_owned.len() == mu.combo_members.len() - 1 {
                out.push(improve(6, Some(si), Some(room_id.clone()), mu.name,
                    &format!("多用法干员以散件形态使用；{}（{}）可对照组合形态，以完整周期班次比较总效率", mu.combo_form, mu.note),
                    "skill-9 检查 6（多组合联合优化对照）"));
            }
        }
    }
}

// ---- 检查 7：心情消耗与周期可持续 ----

fn check7_mood(shifts: &[Shift], views: &[ShiftView], layout: &LayoutInfo, out: &mut Vec<Finding>) {
    let total_hours: f64 = shifts.iter().map(|s| s.duration_hours).sum();
    let recovery = layout.min_dorm_recovery();
    let mut work: HashMap<String, Vec<usize>> = HashMap::new();
    let mut room_of: HashMap<(usize, String), String> = HashMap::new();
    for (si, view) in views.iter().enumerate() {
        for (name, room_id) in &view.duty {
            work.entry(name.clone()).or_default().push(si);
            room_of.insert((si, name.clone()), room_id.clone());
        }
    }
    for (name, shift_ids) in &work {
        let mut sorted = shift_ids.clone();
        sorted.sort_unstable();
        // 最长连续在岗段
        let mut best_len = 1usize;
        let mut run = 1usize;
        for w in sorted.windows(2) {
            if w[1] == w[0] + 1 {
                run += 1;
                best_len = best_len.max(run);
            } else {
                run = 1;
            }
        }
        let mut longest_hours = 0.0f64;
        {
            let mut run_hours = 0.0;
            let mut prev: Option<usize> = None;
            for &si in &sorted {
                if prev == Some(si.saturating_sub(1)) {
                    run_hours += shifts[si].duration_hours;
                } else {
                    run_hours = shifts[si].duration_hours;
                }
                longest_hours = longest_hours.max(run_hours);
                prev = Some(si);
            }
        }
        // 净消耗（取所在房间的最高消耗：按该干员各班中最不利的房间）
        let mut worst_cost = 0.0;
        let mut worst_room = String::new();
        for &si in &sorted {
            let room_id = room_of
                .get(&(si, name.clone()))
                .cloned()
                .unwrap_or_default();
            let kind = if room_id.starts_with("trade") {
                "trade"
            } else if room_id.starts_with("manu") {
                "manufacture"
            } else {
                "other"
            };
            let (occ, level) = {
                let room = views[si].rooms.iter().find(|r| r.room_id == room_id);
                let occ = room.map(|r| r.operators.len()).unwrap_or(1);
                let level = layout.level_of(&room_id);
                (occ, level)
            };
            let cost = if kind == "other" {
                0.75
            } else {
                base_mood_cost(kind, occ, level)
            };
            if cost > worst_cost {
                worst_cost = cost;
                worst_room = room_id;
            }
        }
        let delta = MOOD_DELTA_TABLE
            .iter()
            .find(|d| d.name == name)
            .map(|d| d.extra_per_hour)
            .unwrap_or(0.0);
        let net = worst_cost + delta;
        // 单管上限
        if longest_hours * net > 24.0 {
            out.push(error(7, None, Some(worst_room.clone()), name,
                &format!("最长连续在岗 {longest_hours:.0}h × 净消耗 {net:.2}/h = {:.1} 点，超过一管 24 点，班内涣散、技能失效", longest_hours * net),
                "心情与宿舍·工休比与最长工作时间"));
        }
        // 周期平衡
        let worked_hours: f64 = sorted.iter().map(|&si| shifts[si].duration_hours).sum();
        let rest_hours = (total_hours - worked_hours).max(0.0);
        let consumed = worked_hours * net;
        let recovered = rest_hours * recovery;
        if consumed > recovered + 1e-6 {
            out.push(error(7, None, None, name,
                &format!("周期不可持续：工 {worked_hours:.0}h 耗 {consumed:.1} 点 > 休 {rest_hours:.0}h 回 {recovered:.1} 点（宿舍回复 {recovery}/h）"),
                "心情与宿舍·工休比（连续在岗时长×净消耗 ≤ 回复×休息时间）"));
        }
        let _ = best_len;
    }
}

// ---- 检查 8：连续全周期工作 ----

fn check8_continuous(
    shifts: &[Shift],
    views: &[ShiftView],
    fiam: &[(usize, String)],
    out: &mut Vec<Finding>,
) {
    let n = shifts.len();
    let mut work: HashMap<String, usize> = HashMap::new();
    let mut dorm_only: HashMap<String, usize> = HashMap::new();
    let mut room_of: HashMap<(usize, String), String> = HashMap::new();
    for (si, view) in views.iter().enumerate() {
        for (name, room_id) in &view.duty {
            *work.entry(name.clone()).or_default() += 1;
            room_of.insert((si, name.clone()), room_id.clone());
        }
        for name in view.dorm.keys() {
            *dorm_only.entry(name.clone()).or_default() += 1;
        }
    }
    let fiam_names: HashSet<&str> = fiam.iter().map(|(_, t)| t.as_str()).collect();
    for (name, count) in &work {
        if *count < n {
            continue;
        }
        // 豁免名单
        if fiam_names.contains(name.as_str()) {
            out.push(note(
                8,
                None,
                None,
                name,
                "菲亚梅塔换心情目标，连续全周期工作属豁免（须核输送容量 ≤2/h）",
                "排班机制·三班全勤与跨班继承；菲亚梅塔·生效条件",
            ));
            continue;
        }
        if CONTINUOUS_EXEMPT.contains(&name.as_str()) {
            continue; // Lancet-2 / 彩虹小队等
        }
        let rooms: Vec<String> = (0..n)
            .filter_map(|si| room_of.get(&(si, name.clone())).cloned())
            .collect();
        let uniq: HashSet<&String> = rooms.iter().collect();
        out.push(error(
            8,
            None,
            None,
            name,
            &format!(
                "跨全部 {} 班连续在岗（{}），周期不可持续、必然涣散",
                n,
                uniq.into_iter().cloned().collect::<Vec<_>>().join("→")
            ),
            "心情与宿舍·心情的基础规则",
        ));
    }
    // 宿舍常驻（宿管/生产端）提示
    for (name, count) in &dorm_only {
        if *count == n && !work.contains_key(name) {
            out.push(note(
                8,
                None,
                None,
                name,
                "全周期驻宿舍（宿管/宿舍生产端），自身不消耗心情，属正常形态",
                "心情与宿舍·宿舍",
            ));
        }
    }
}

// ---- 检查 9：挂件同进退 ----

fn check9_pendants(_shifts: &[Shift], views: &[ShiftView], out: &mut Vec<Finding>) {
    for (si, view) in views.iter().enumerate() {
        for pair in PENDANT_PAIRS {
            let pendant_duty_room = view.duty.get(pair.pendant);
            let Some(room_id) = pendant_duty_room else {
                continue;
            };
            // 森西是宿舍核心：消费端在班即要求森西在宿舍
            let core_ok = if pair.core_room == "dorm" {
                view.dorm.keys().any(|k| pair.core.contains(&k.as_str()))
            } else if pair.core_room == "control" {
                pair.core
                    .iter()
                    .all(|c| view.duty.get(*c).map(|r| r == "control").unwrap_or(false))
            } else if pair.core_room == "trade" {
                pair.core.iter().all(|c| {
                    view.duty
                        .get(*c)
                        .map(|r| r.starts_with("trade"))
                        .unwrap_or(false)
                })
            } else {
                // “在基建内”包括工作房和宿舍；宿舍成员不在 duty 索引中。
                pair.core
                    .iter()
                    .all(|c| view.duty.contains_key(*c) || view.dorm.contains_key(*c))
            };
            if !core_ok {
                out.push(error(
                    9,
                    Some(si),
                    Some(room_id.clone()),
                    pair.pendant,
                    &format!("挂件与核心错班：{}（{}）", pair.note, pair.core.join("/")),
                    "skill-9 检查 9（挂件依赖对）",
                ));
            }
        }
    }
}

// ---- 检查 10：效率与产出复算 ----

fn check10_recalc(result: &PlanResult, layout: &LayoutInfo, out: &mut Vec<Finding>) {
    let rotation = &result.rotation;
    let total_hours: f64 = rotation.shifts.iter().map(|s| s.duration_hours).sum();
    if total_hours <= 0.0 {
        return;
    }
    let mut lmd = 0.0;
    let mut gold = 0.0;
    let mut exp = 0.0;
    let mut shards = 0.0;
    let mut orundum = 0.0;
    let mut trade_w = 0.0;
    let mut manu_w = 0.0;
    let mut power_w = 0.0;
    for shift in &rotation.shifts {
        let w = shift.duration_hours / total_hours;
        trade_w += shift.weighted_trade;
        manu_w += shift.weighted_manufacture;
        power_w += shift.weighted_power;
        for line in &shift.efficiencies.room_lines {
            let kind = room_kind(&line.room_id);
            match kind {
                "trade" => {
                    if layout.trade_order(&line.room_id) == "originium" {
                        orundum += line.trade_output_per_day * w;
                    } else {
                        lmd += line.trade_output_per_day * w;
                    }
                }
                "manufacture" => match line.manufacture_unit_output_per_day as i64 {
                    10000 => gold += line.manufacture_output_per_day * w,
                    8000 => exp += line.manufacture_output_per_day * w,
                    24 => shards += line.manufacture_output_per_day * w,
                    _ => {}
                },
                _ => {}
            }
        }
    }
    let daily = &rotation.daily;
    let mut compare = |name: &str, recomputed: f64, reported: f64| {
        if recomputed > 0.0 && (recomputed - reported).abs() / recomputed.max(1.0) > 0.01 {
            out.push(error(
                10,
                None,
                None,
                name,
                &format!("日汇总复算不一致：逐班加权 {recomputed:.0} vs 报表 {reported:.0}"),
                "产出常数表·产出总公式",
            ));
        }
    };
    compare("lmd", lmd, daily.production.lmd);
    compare("pure_gold", gold, daily.production.pure_gold);
    compare("battle_records", exp, daily.production.battle_records);
    compare("orundum", orundum, daily.production.orundum);
    compare(
        "originium_shards",
        shards,
        daily.production.originium_shards,
    );
    compare("trade_efficiency", trade_w, daily.trade);
    compare("manufacture_efficiency", manu_w, daily.manufacture);
    compare("power_efficiency", power_w, daily.power);
    // 逐房结构复算：total ≈ base（1.0+进驻）+ skill + global
    for shift in &rotation.shifts {
        for line in &shift.efficiencies.room_lines {
            let kind = room_kind(&line.room_id);
            if kind != "trade" && kind != "manufacture" {
                continue;
            }
            let recomputed =
                line.base_efficiency + line.operator_efficiency + line.global_efficiency;
            if (recomputed - line.total_efficiency).abs() > 0.005 {
                out.push(error(10, Some(shift.index), Some(line.room_id.clone()), line.room_id.as_str(),
                    &format!("房间效率构成复算不一致：base{:.3}+技能{:.3}+光环{:.3}={:.3} vs total {:.3}",
                        line.base_efficiency, line.operator_efficiency, line.global_efficiency, recomputed, line.total_efficiency),
                    "基建生产关系·从效率到产出（总效率构成）"));
            }
        }
    }
}

// ---- 检查 11：无人机投向 ----

fn check11_drones(result: &PlanResult, drones: &[(usize, String, i64)], out: &mut Vec<Finding>) {
    if drones.is_empty() {
        out.push(note(
            11,
            None,
            None,
            "drones",
            "排班表未启用无人机加速（或未配置投向）",
            "无人机使用·加速贸易站的优先级",
        ));
        return;
    }
    for (si, room, index) in drones {
        let Some(shift) = result.rotation.shifts.get(*si) else {
            continue;
        };
        // 候选：本班贸易房按（倍率、日产出）排序
        let mut trades: Vec<&RoomLine> = shift
            .efficiencies
            .room_lines
            .iter()
            .filter(|l| room_kind(&l.room_id) == "trade")
            .collect();
        trades.sort_by(|a, b| {
            b.order_multiplier
                .partial_cmp(&a.order_multiplier)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    b.trade_output_per_day
                        .partial_cmp(&a.trade_output_per_day)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });
        let Some(best) = trades.first() else { continue };
        // 目标房间映射：room=trading → 第 index 个贸易房
        let target = if room == "trading" || room == "trade" {
            trades
                .get((*index as usize).saturating_sub(1))
                .map(|l| l.room_id.clone())
        } else if room == "manufacture" {
            None // 制造投向按产物配平判断，另走产线配平
        } else {
            None
        };
        if room == "manufacture" {
            out.push(note(11, Some(*si), None, "drones→manufacture",
                "无人机投制造：按产物短板配平（赤金不够补赤金、钱书缺口补短板）；同类普通产物之间加谁收益都一样",
                "无人机使用·总原则"));
            continue;
        }
        if let Some(t) = target {
            if t != best.room_id
                && best.order_multiplier
                    > trades
                        .iter()
                        .find(|l| l.room_id == t)
                        .map(|l| l.order_multiplier)
                        .unwrap_or(0.0)
                        + 1e-9
            {
                out.push(improve(
                    11,
                    Some(*si),
                    Some(t.clone()),
                    t.as_str(),
                    &format!(
                        "加速贸易站按七级优先级应优先 {}（倍率 {:.3}），当前投向 {}（倍率 {:.3}）",
                        best.room_id,
                        best.order_multiplier,
                        t,
                        trades
                            .iter()
                            .find(|l| l.room_id == t)
                            .map(|l| l.order_multiplier)
                            .unwrap_or(0.0)
                    ),
                    "无人机使用·加速贸易站的优先级",
                ));
            }
        }
    }
}

// ---- 检查 12：办公室感染力失效 ----

fn check12_charm(views: &[ShiftView], out: &mut Vec<Finding>) {
    for (si, view) in views.iter().enumerate() {
        let in_control = |n: &str| view.duty.get(n).map(|r| r == "control").unwrap_or(false);
        if in_control("琴柳") && (in_control("八幡海铃") || in_control("焰狐龙梓兰")) {
            let office_ops: Vec<&String> = view
                .duty
                .iter()
                .filter(|(_, r)| r.starts_with("office"))
                .map(|(n, _)| n)
                .collect();
            let has_skill = office_ops.iter().any(|n| {
                OFFICE_TABLE.iter().any(|e| e.name == n.as_str())
                    || OFFICE_COMBO_EXEMPT.contains(&n.as_str())
            });
            if has_skill {
                out.push(error(12, Some(si), Some("control".into()), "琴柳×海铃/龙梓兰",
                    "琴柳感染力与海铃/龙梓兰同驻中枢时，办公室干员联络效率 ≥15% 即整条失效（联络散件普遍 45%+，几乎必然越线）",
                    "中枢席位取舍·同驻边界；感染力（释义）"));
            }
        }
    }
}

// ---- 检查 13：技能与设施产物不符 ----

fn check13_product(layout: &LayoutInfo, views: &[ShiftView], out: &mut Vec<Finding>) {
    for (si, view) in views.iter().enumerate() {
        for room in view.rooms {
            let kind = layout.kind_of(&room.room_id);
            let names: Vec<&str> = room.operators.iter().map(|o| o.name.as_str()).collect();
            if kind.contains("factory") {
                let recipe = layout.manu_recipe(&room.room_id);
                for n in &names {
                    if recipe == "battle_record"
                        && SKILL_GOLD_ONLY.contains(n)
                        && !SKILL_EXP_ONLY.contains(n)
                    {
                        out.push(error(
                            13,
                            Some(si),
                            Some(room.room_id.clone()),
                            *n,
                            "金属工艺/贵金属类技能只作用于赤金配方，经验站内技能空转",
                            "制造站机制·生产力类",
                        ));
                    }
                    if recipe == "gold"
                        && SKILL_EXP_ONLY.contains(n)
                        && !SKILL_GOLD_ONLY.contains(n)
                    {
                        out.push(error(
                            13,
                            Some(si),
                            Some(room.room_id.clone()),
                            *n,
                            "作战记录类技能只作用于经验配方，赤金站内技能空转",
                            "制造站机制·生产力类",
                        ));
                    }
                    if recipe != "originium" && SKILL_SHARD_ONLY.contains(n) {
                        out.push(improve(
                            13,
                            Some(si),
                            Some(room.room_id.clone()),
                            *n,
                            "地质学/源石工艺类技能只对源石碎片配方生效，非搓玉站内技能空转",
                            "制造站机制·生产力类",
                        ));
                    }
                }
            }
        }
    }
}

// ---- 检查 14：办公室与会客室散件满足度 ----

fn check14_office_meeting(
    box_data: &OperBox,
    views: &[ShiftView],
    registry: &ComboRegistry,
    out: &mut Vec<Finding>,
) {
    // 挂件类干员（任一组合的 pendant/core_dorm 成员）按组合功能评估，不按散件效率报改进
    let pendant_names: HashSet<&str> = registry
        .combos
        .iter()
        .flat_map(|c| c.members.iter())
        .filter(|m| m.role.contains("pendant") || m.role.contains("dorm"))
        .map(|m| m.name.as_str())
        .collect();
    for (si, view) in views.iter().enumerate() {
        // 办公室
        let office_ops: Vec<&String> = view
            .duty
            .iter()
            .filter(|(_, r)| r.starts_with("office"))
            .map(|(n, _)| n)
            .collect();
        for op in &office_ops {
            if OFFICE_COMBO_EXEMPT.contains(&op.as_str()) || pendant_names.contains(op.as_str()) {
                continue;
            }
            let current = OFFICE_TABLE
                .iter()
                .filter(|e| {
                    box_data.owned(e.name)
                        && box_data
                            .get(e.name)
                            .map(|b| b.elite >= e.elite_req)
                            .unwrap_or(false)
                })
                .map(|e| e.speed_pct)
                .max()
                .unwrap_or(0);
            let mine = OFFICE_TABLE
                .iter()
                .find(|e| {
                    e.name == op.as_str()
                        && box_data
                            .get(e.name)
                            .map(|b| b.elite >= e.elite_req)
                            .unwrap_or(true)
                })
                .map(|e| e.speed_pct)
                .unwrap_or(-1);
            if mine >= 0 && current > mine + 5 {
                let best_name = OFFICE_TABLE
                    .iter()
                    .filter(|e| {
                        box_data.owned(e.name)
                            && box_data
                                .get(e.name)
                                .map(|b| b.elite >= e.elite_req)
                                .unwrap_or(false)
                            && e.speed_pct == current
                    })
                    .map(|e| e.name)
                    .next()
                    .unwrap_or("");
                out.push(improve(
                    14,
                    Some(si),
                    Some("office".into()),
                    op.as_str(),
                    &format!(
                        "办公室散件满足度不足：当前 {mine}%，box 可达 {current}%（如 {best_name}）"
                    ),
                    "高效率散件·办公室（参考线 45%）",
                ));
            }
        }
        // 会客室：合计口径（技能% + 星级% + 精英化%）；保留在会干员中最优者，只对比可替换席
        let meeting_ops: Vec<&ShiftOperator> = view
            .rooms
            .iter()
            .filter(|r| r.room_id == "meeting")
            .flat_map(|r| r.operators.iter())
            .collect();
        if meeting_ops.is_empty() {
            continue;
        }
        let value_of = |o: &ShiftOperator| -> i32 {
            let skill = MEETING_TABLE
                .iter()
                .find(|e| e.name == o.name && o.elite >= e.elite_req)
                .map(|e| e.skill_pct)
                .unwrap_or(0);
            skill + meeting_station_bonus(o.rarity, o.elite)
        };
        let actual_total: i32 = meeting_ops.iter().map(|o| value_of(o)).sum();
        let mut exempt = false;
        for o in &meeting_ops {
            if pendant_names.contains(o.name.as_str()) {
                exempt = true;
            }
        }
        if exempt {
            continue; // 会客室有组合挂件（如 S.E.E.S. 计数），按组合功能评估
        }
        let in_meeting: HashSet<&str> = meeting_ops.iter().map(|o| o.name.as_str()).collect();
        let best_actual = meeting_ops.iter().map(|o| value_of(o)).max().unwrap_or(0);
        let best_replacement = MEETING_TABLE
            .iter()
            .filter_map(|e| {
                let b = box_data.get(e.name)?;
                if !b.own
                    || b.elite < e.elite_req
                    || in_meeting.contains(e.name)
                    || pendant_names.contains(e.name)
                {
                    return None;
                }
                Some(e.skill_pct + meeting_station_bonus(b.rarity, b.elite))
            })
            .max()
            .unwrap_or(0);
        let achievable = best_actual + best_replacement;
        if achievable > actual_total + 5 {
            out.push(improve(14, Some(si), Some("meeting".into()), "meeting",
                    &format!("会客室合计口径（技能+星级+精英化）当前 {actual_total}%，替换空弱席位后可达约 {achievable}%"),
                    "会客室机制·线索搜集速度；会客室取向·核心效率人"));
        }
    }
}

// ---- 检查 15：练卡建议与 profile 回显一致性 ----

fn check15_result_metadata(result: &PlanResult, registry: &ComboRegistry, out: &mut Vec<Finding>) {
    let known_ids: HashSet<&str> = registry.combos.iter().map(|c| c.id.as_str()).collect();
    let advice = &result.training_advice;
    if advice.schema_version != 0 && advice.schema_version != 2 {
        out.push(error(
            15,
            None,
            None,
            "training_advice",
            &format!(
                "练卡建议 schema_version={}，当前契约为 2",
                advice.schema_version
            ),
            "练卡推荐兼容验收·schema_version 2",
        ));
    }
    for combo in &advice.combinations {
        let known = known_ids.contains(combo.id.as_str())
            || registry.combos.iter().any(|c| {
                c.name == combo.id
                    || (!combo.name.is_empty() && c.name == combo.name)
                    || c.aliases.iter().any(|alias| alias == &combo.id)
            });
        if !known {
            out.push(note(
                15,
                None,
                None,
                &combo.id,
                "练卡建议引用了组合登记表之外的 id；可作为外部扩展，但不会进入本工具的效率核对",
                "练卡推荐兼容验收·组合 id",
            ));
        }
        if !(0..=100).contains(&combo.completion_percent) || combo.completed_slots < 0 {
            out.push(error(
                15,
                None,
                None,
                &combo.id,
                &format!(
                    "组合完成度字段越界：completed_slots={} completion_percent={}",
                    combo.completed_slots, combo.completion_percent
                ),
                "练卡推荐兼容验收·完成度字段",
            ));
        }
    }
    let Some(rotation) = result.profile.get("rotation") else {
        return;
    };
    let pairs = [
        ("daily_trade_efficiency", result.rotation.daily.trade),
        (
            "daily_manufacture_efficiency",
            result.rotation.daily.manufacture,
        ),
        ("daily_power_efficiency", result.rotation.daily.power),
    ];
    for (field, expected) in pairs {
        if let Some(actual) = rotation.get(field).and_then(|v| v.as_f64()) {
            if (actual - expected).abs() > 0.005 {
                out.push(error(
                    15,
                    None,
                    None,
                    field,
                    &format!(
                        "profile.rotation 与 rotation.daily 不一致：{actual:.3} vs {expected:.3}"
                    ),
                    "plan-compute-v4·profile 回显",
                ));
            }
        }
    }
}
