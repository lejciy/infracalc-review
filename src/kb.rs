//! 知识库静态登记表：中枢光环家族、互斥名单、产物技能分类、办公室/会客室散件、
//! 心情增耗、挂件依赖对。全部编码自 arknights-base-vault `docs/` 正文（kb_ref 注明出处）。

use serde::Deserialize;

// ---- 组合登记表（data/combos.json，编译期嵌入） ----

#[derive(Debug, Deserialize)]
pub struct ComboRegistry {
    pub combos: Vec<Combo>,
}

#[derive(Debug, Deserialize)]
pub struct Combo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub layer: String,
    pub product: String,
    pub kb_ref: String,
    pub members: Vec<ComboMember>,
    pub eval: Option<ComboEval>,
    pub anchor: ComboAnchor,
}

#[derive(Debug, Deserialize)]
pub struct ComboMember {
    pub name: String,
    pub elite: u8,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Deserialize)]
pub struct ComboEval {
    pub rooms: Vec<ComboEvalRoom>,
    /// "sum"＝锚点为多个生产房合计（如深海 2+2）；缺省＝取锚点房
    #[serde(default)]
    pub aggregate: String,
    /// 锚点房在 rooms 中的下标；缺省＝第一个生产房（按规格序）
    #[serde(default)]
    pub anchor_room: Option<usize>,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Deserialize)]
pub struct ComboEvalRoom {
    pub room_id: String,
    #[serde(default)]
    pub level: Option<u8>,
    #[serde(default)]
    pub product: Option<String>,
    pub operators: Vec<String>,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Deserialize)]
pub struct ComboAnchor {
    pub metric: String,
    pub value: f64,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub note: String,
}

pub fn load_registry() -> Result<ComboRegistry, String> {
    serde_json::from_str(include_str!("../data/combos.json"))
        .map_err(|e| format!("combos.json parse: {e}"))
}

// ---- 中枢光环家族（同种取最高；docs/1-基础设定/设施与进驻/控制中枢机制.md·全局效率光环） ----

pub struct AuraFamily {
    pub family: &'static str,
    pub members: &'static [&'static str],
    pub kb_ref: &'static str,
}

pub const AURA_FAMILIES: &[AuraFamily] = &[
    AuraFamily {
        family: "trade_global_7", // 所有贸易站订单效率 +7% 类
        members: &[
            "阿米娅",
            "诗怀雅",
            "阿斯卡纶",
            "明椒",
            "火龙S黑角",
            "若叶睦",
        ],
        kb_ref: "控制中枢机制·全局效率光环",
    },
    AuraFamily {
        family: "manufacture_global", // 所有制造站生产力 +x% 类
        members: &["凯尔希", "Mon3tr", "布丁", "斩业星熊", "麒麟R夜刀"],
        kb_ref: "控制中枢机制·全局效率光环",
    },
    AuraFamily {
        family: "office_link_10", // 人力办公室联络速度 +10%
        members: &["八幡海铃", "焰狐龙梓兰"],
        kb_ref: "控制中枢机制·全局效率光环",
    },
    AuraFamily {
        family: "meeting_collect", // 线索搜集速度（世事洞明25/期冀之汇15/勤学苦练5）
        members: &["老鲤", "魔王", "祐天寺若麦"],
        kb_ref: "控制中枢机制·全局效率光环",
    },
    AuraFamily {
        family: "training_5", // 训练室专精速度 +5%
        members: &["烛煌", "斩业星熊", "阿斯卡纶"],
        kb_ref: "控制中枢机制·全局效率光环",
    },
];

/// 心情恢复具名比较：互不叠加、取最高（心情与宿舍·减免来源与叠加）
pub const MOOD_RECOVERY_NAMED: &[&str] = &["玛恩纳", "重岳", "维什戴尔"];

// ---- 贸易特殊订单优先级（贸易站机制·特殊订单） ----

pub const ORDER_REWRITERS: &[(&str, u8)] = &[
    ("佩佩", 5),
    ("可露希尔", 4),
    ("U-Official", 3),
    ("但书", 2),
    ("龙舌兰", 1),
];

/// 裁缝 β 持有者（精二时 α 升级为 β；龙舌兰组·成员与档位）
pub const TAILOR_BETA_HOLDERS: &[&str] = &["卡夫卡", "柏喙", "明椒", "折光"];

// ---- 制造归零类与互斥（自动化.md·易误读点；组合总览·互斥规则） ----

/// 归零类技能持有者：进驻制造站时清零其他干员的常规生产力
pub const MANU_ZEROERS: &[&str] = &["温蒂", "森蚺", "异客", "掠风", "冬时"];
/// 与归零类可同站共存的设施数量类/同款清零技能
pub const MANU_ZERO_EXEMPT: &[&str] = &["温蒂", "森蚺", "异客", "掠风", "冬时", "清流"];
/// 深海猎人技能判定名单（组合总览·互斥规则：深海×自动化、深海×槐琥）
pub const ABYSSAL_HUNTERS: &[&str] = &["乌尔比安", "斯卡蒂", "幽灵鲨", "安哲拉"];

// ---- 技能产物分类（制造站机制·生产力类；高效率散件各产物节） ----

/// 金属工艺/贵金属类：只作用于赤金配方（莱茵科技/标准化/自动化/仓容转化类为通用，不在此列）
pub const SKILL_GOLD_ONLY: &[&str] = &["砾", "苍苔", "斑点", "夜烟", "引星棘刺", "清流"];
/// 作战记录类：只作用于经验配方（通用类不在此列：水月/香草/杰西卡标准化、莱茵科技、泡泡火神仓容）
pub const SKILL_EXP_ONLY: &[&str] = &[
    "弑君者",
    "食铁兽",
    "断罪者",
    "裂响",
    "机械师",
    "灰毫",
    "远牙",
    "野鬃",
    "怒潮凛冬",
    "烈夏",
    "酒神",
    "稀音",
    "帕拉斯",
    "刻俄柏",
];
/// 源石碎片类：只作用于搓玉线
pub const SKILL_SHARD_ONLY: &[&str] = &[
    "褐果",
    "炎熔",
    "地灵",
    "谬因",
    "艾雅法拉",
    "锡兰",
    "薄绿",
    "月见夜",
];

// ---- 办公室散件（高效率散件·办公室，参考线 45%） ----

pub struct OfficeEntry {
    pub name: &'static str,
    pub elite_req: u8,
    pub speed_pct: i32,
    pub mood_cost: f64,
    pub cond: &'static str,
}

pub const OFFICE_TABLE: &[OfficeEntry] = &[
    OfficeEntry {
        name: "斥罪",
        elite_req: 0,
        speed_pct: 50,
        mood_cost: 0.5,
        cond: "",
    },
    OfficeEntry {
        name: "珊比",
        elite_req: 2,
        speed_pct: 45,
        mood_cost: 0.0,
        cond: "",
    },
    OfficeEntry {
        name: "艾雅法拉",
        elite_req: 2,
        speed_pct: 45,
        mood_cost: 0.0,
        cond: "",
    },
    OfficeEntry {
        name: "遥",
        elite_req: 2,
        speed_pct: 45,
        mood_cost: 0.0,
        cond: "",
    },
    OfficeEntry {
        name: "普罗旺斯",
        elite_req: 2,
        speed_pct: 45,
        mood_cost: 0.0,
        cond: "",
    },
    OfficeEntry {
        name: "水灯心",
        elite_req: 2,
        speed_pct: 45,
        mood_cost: 1.0,
        cond: "",
    },
    OfficeEntry {
        name: "地灵",
        elite_req: 1,
        speed_pct: 45,
        mood_cost: 2.0,
        cond: "",
    },
    OfficeEntry {
        name: "锡人",
        elite_req: 2,
        speed_pct: 45,
        mood_cost: 0.0,
        cond: "满宿舍总20级",
    },
];

/// 办公室组合角色（按组合功能评估，不按散件效率报改进）
pub const OFFICE_COMBO_EXEMPT: &[&str] = &["絮雨", "桑葚", "凯尔希·思衡托"];

// ---- 会客室（会客室取向·核心效率人；会客室机制·线索搜集速度） ----

pub struct MeetingEntry {
    pub name: &'static str,
    pub elite_req: u8,
    pub skill_pct: i32,
    pub note: &'static str,
}

pub const MEETING_TABLE: &[MeetingEntry] = &[
    MeetingEntry {
        name: "信仰搅拌机",
        elite_req: 2,
        skill_pct: 20,
        note: "菲亚梅塔进驻宿舍时+10%（不泯童心）",
    },
    MeetingEntry {
        name: "伊内丝",
        elite_req: 2,
        skill_pct: 20,
        note: "聚影每小时+2%封顶30%（长班更高）",
    },
    MeetingEntry {
        name: "跃跃",
        elite_req: 1,
        skill_pct: 30,
        note: "仅线索交流期生效；精0只有10%",
    },
    MeetingEntry {
        name: "伺夜",
        elite_req: 2,
        skill_pct: 25,
        note: "领袖外交",
    },
    MeetingEntry {
        name: "陈",
        elite_req: 2,
        skill_pct: 25,
        note: "警司",
    },
    MeetingEntry {
        name: "赤刃明霄陈",
        elite_req: 2,
        skill_pct: 25,
        note: "警司同款",
    },
    MeetingEntry {
        name: "见行者",
        elite_req: 1,
        skill_pct: 35,
        note: "心情+2/h，长班覆盖率差不推荐常驻",
    },
    MeetingEntry {
        name: "忍冬",
        elite_req: 2,
        skill_pct: 20,
        note: "与铃兰同室额外+30%（杀手的假期）",
    },
    MeetingEntry {
        name: "铃兰",
        elite_req: 0,
        skill_pct: 0,
        note: "自身无速度技能，作为忍冬搭档",
    },
    MeetingEntry {
        name: "凛视",
        elite_req: 2,
        skill_pct: 10,
        note: "与提丰同室+15%（未来之途）",
    },
    MeetingEntry {
        name: "提丰",
        elite_req: 0,
        skill_pct: 10,
        note: "冰原游弋，与萨米干员同室再+5%",
    },
];

/// 会客室进驻加成（每名干员各自一份）：星级 + 精英化（会客室机制·线索搜集速度）
pub fn meeting_station_bonus(rarity: u8, elite: u8) -> i32 {
    let by_rarity = match rarity {
        4 => 2,
        5 => 4,
        6 => 5,
        _ => 0,
    };
    let by_elite = match elite {
        1 => 8,
        2 => 16,
        _ => 0,
    };
    by_rarity + by_elite
}

/// 会客室固定搭档（须同室且同班）
pub const MEETING_PARTNERS: &[(&str, &str)] = &[("忍冬", "铃兰"), ("凛视", "提丰")];

// ---- 心情增耗（心情与宿舍·减免来源；错班轮换·单站与多人站班型） ----

pub struct MoodDelta {
    pub name: &'static str,
    pub extra_per_hour: f64,
    pub note: &'static str,
}

pub const MOOD_DELTA_TABLE: &[MoodDelta] = &[
    MoodDelta {
        name: "歌蕾蒂娅",
        extra_per_hour: 2.5,
        note: "潮汐守望：宿舍外深海猎人每人+0.5（满配5人=2.5），单班≤7.5h",
    },
    MoodDelta {
        name: "斥罪",
        extra_per_hour: 0.5,
        note: "法为正典（办公室净耗1.25）",
    },
    MoodDelta {
        name: "阿罗玛",
        extra_per_hour: 0.25,
        note: "净味香氛",
    },
    MoodDelta {
        name: "裂响",
        extra_per_hour: 0.25,
        note: "连轴转",
    },
    MoodDelta {
        name: "见行者",
        extra_per_hour: 2.0,
        note: "逻辑推理（会客室）",
    },
    MoodDelta {
        name: "地灵",
        extra_per_hour: 2.0,
        note: "准时下班（办公室）",
    },
    MoodDelta {
        name: "水灯心",
        extra_per_hour: 1.0,
        note: "永不停歇（办公室）",
    },
];

/// 设施类型净消耗基准（满中枢减免后；心情与宿舍·心情消耗）
pub fn base_mood_cost(kind: &str, occupancy: usize, level: u8) -> f64 {
    let facility_relief = match (kind, occupancy, level) {
        ("trade", 3, 3) | ("manufacture", 3, 3) => 0.10,
        ("trade", 2, 2) | ("manufacture", 2, 2) => 0.05,
        _ => 0.0,
    };
    1.0 - 0.25 - facility_relief
}

/// 宿舍满氛围合计回复（心情与宿舍·恢复公式）
pub fn dorm_recovery(level: u8) -> f64 {
    match level {
        1 => 2.0,
        2 => 2.5,
        3 => 3.0,
        4 => 3.5,
        _ => 4.0,
    }
}

// ---- 挂件依赖对（检查 9；各组合深稿） ----

pub struct PendantPair {
    pub pendant: &'static str,
    pub core: &'static [&'static str],
    pub core_room: &'static str,
    pub note: &'static str,
}

pub const PENDANT_PAIRS: &[PendantPair] = &[
    PendantPair {
        pendant: "泰拉大陆调查团",
        core: &["火龙S黑角", "麒麟R夜刀"],
        core_room: "control",
        note: "调查团须与怪猎中枢二人同班",
    },
    PendantPair {
        pendant: "Lancet-2",
        core: &["森蚺"],
        core_room: "control",
        note: "森蚺中枢分支触发件，红脸驻发电站",
    },
    PendantPair {
        pendant: "烈夏",
        core: &["古米"],
        core_room: "trade",
        note: "烈夏35%档需古米驻贸易站（熊团经验组）",
    },
    PendantPair {
        pendant: "玛露西尔",
        core: &["森西"],
        core_room: "dorm",
        note: "魔物料理消费端在班时森西须驻宿舍产料",
    },
    PendantPair {
        pendant: "莱欧斯",
        core: &["森西"],
        core_room: "dorm",
        note: "同上（会客侧）",
    },
    PendantPair {
        pendant: "齐尔查克",
        core: &["森西"],
        core_room: "dorm",
        note: "同上（贸易侧）",
    },
    PendantPair {
        pendant: "深巡",
        core: &["乌尔比安"],
        core_room: "any",
        note: "乌尔比安在基建内时深巡+10%",
    },
    PendantPair {
        pendant: "绮良",
        core: &["鸿雪", "图耶"],
        core_room: "trade",
        note: "鸿雪杜林组核心同班（虚拟产线另计）",
    },
];

// ---- 菲亚梅塔输送容量与豁免（菲亚梅塔.md·生效条件） ----

pub const FIAMMETTA_CAPACITY: f64 = 2.0;
pub const CONTINUOUS_EXEMPT: &[&str] = &["Lancet-2", "灰烬", "战车", "闪击", "霜华", "艾拉"];

// ---- 多用法干员（检查 6；公孙长乐 2026-09-26 口径） ----

pub struct MultiUse {
    pub name: &'static str,
    pub combo_form: &'static str,
    pub combo_members: &'static [&'static str],
    pub note: &'static str,
}

pub const MULTI_USE: &[MultiUse] = &[
    MultiUse {
        name: "酒神",
        combo_form: "红云组",
        combo_members: &["红云", "酒神", "Miss.Christine"],
        note: "散件35% vs 红云组经验分支满配+106%",
    },
    MultiUse {
        name: "水月",
        combo_form: "水月标准化组",
        combo_members: &["水月", "香草", "杰西卡"],
        note: "散件 vs 黑钢中枢形态+100%",
    },
    MultiUse {
        name: "苍苔",
        combo_form: "赤金工艺组",
        combo_members: &["苍苔", "引星棘刺", "砾"],
        note: "单走35% vs 三人组+110%",
    },
    MultiUse {
        name: "多萝西",
        combo_form: "莱茵科技",
        combo_members: &["多萝西", "淬羽赫默", "娜斯提"],
        note: "散件 vs 三人组+95%",
    },
];
