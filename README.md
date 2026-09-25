# infracalc-review

排班表审查与组合效率核对工具（Rust）。输入干员盒（operbox）与排班工具的
`plan.compute` 结果 JSON（含轮换排班、MAA 导出、练卡建议、box 对标），输出：

1. **十四项检查**（skill-9 排班表审查口径）：明确错误 / 需要改进 两级结论；
2. **全部组合体系核对**：知识库效率锚点 vs ArknightsInfraCalc-v3 评估函数核算，
   逐组合给出 match / approx / mismatch 判定；
3. **逐班复评**：同一评估函数重放排班表逐房效率，与 plan 自报值对照。

## 依赖

- `../ArknightsInfraCalc-v3`（path 依赖，调用其 `eval_v2_assignment_with_shift`
  效率评估函数；布局文件复用其 `data/layout/*.json`）；
- `data/combos.json`：知识库组合登记表（31 个组合/体系：成员、锚度、部分求解
  房间、效率锚点），编码自 arknights-base-vault `docs/3-组合库/`，出处见各条 `kb_ref`。

## 推荐使用方式

本仓以 path 依赖引用 v3，两仓需克隆为同级目录；`--box` 用你自己的干员盒
JSON，`--plan` 用 v3 对同一 box 求解出的 `plan.compute` 结果 JSON：

```bash
git clone https://github.com/KnightCodeSquareMatrix/ArknightsInfraCalc-v3.git
git clone https://github.com/lejciy/infracalc-review.git
cd infracalc-review
cargo build --offline

# 完整审查（检查 + 组合核对 + 逐班复评）
target/debug/infracalc-review.exe review \
  --layout ../ArknightsInfraCalc-v3/data/layout/243_use_this_.json \
  --box    <box.json> \
  --plan   <plan-compute.json> \
  --out out/report.json --md out/report.md

# 只跑组合核对（可 --only tequila_group 过滤）
target/debug/infracalc-review.exe combos --layout ... --box ... --out out/combos.json

# 任意子集单房评估（部分求解）
target/debug/infracalc-review.exe room --layout ... --box ... \
  --room trade_1 --ops 巫恋,龙舌兰,卡夫卡
```

## 部分求解改造点（相对 v3 `eval` CLI 的全量口径）

`src/partial.rs` 的 `PartialEvaluator`：

1. **子集作业**：只提交目标房间，其余房间留空——任意「部分干员 / 组合 / 体系」
   都可以单独结算；
2. **锚度覆盖**：组合成员按知识库锚点练度评估（合成 operbox 提升精英化档，
   只升不降），解决「box 练度低于组合档位时无法核对该档效率」；
3. **心情注入**：`work_mood=24` 满心情评估，与知识库锚点口径一致；
4. **按配方落房**：组合登记表的房间编号是示意，实际按布局解析同设施、同配方
   房间（产赤金的组合自动落到布局里的赤金配方制造站）；
5. **中枢差值**：global_add 型锚点（Mujica/龙门中枢组）以含/不含中枢两次评估
   的目标房 global 差值核对。

## 样例验证结论（c-540-397 box + 243 plan，2026-09-26）

- 2026-09-26 第二轮（v3 修复后）：组合核对 **21 match / 2 approx / 0 mismatch** /
  2 below_upper_bound / 5 not_owned / 1 manual。四个 mismatch 全部修复并精确命中：
  泡影国 0.75、维什戴尔组 0.40、熊团经验组 0.75、鸿雪挂件第三人档 0.90；
  拉特兰满配 115% 经 v3 CLI 探针验证（真实 box 缺空弦，登记为 not_owned）。
  修复落在 v3 仓（详见下节）；approx 两项为赤金工艺组 1.16（KB 区间 1.10~1.16）
  与红云组 1.01 vs 1.06。
- 首轮发现的 4 个 mismatch 与处置（均已修入 ArknightsInfraCalc-v3，五文件 +273/−11）：
  1. 泡影国狩猎小队——v3 新增 trade_kind 33 的效率结算：每名进驻贸易站的
     泡影国干员 +20%（计数含本人，跨贸易房投影 `hunter_trade_station_count`）；
  2. 维什戴尔组——赫德雷「白手起家」伊内丝/W 各 +5% 联动（α/β 档区分，
     由 `hoederer_extra_pct` 投影）；
  3. 熊团经验组——烈夏「患难拍档」35%：v3 制造运行时本已实现（manu_kind 5），
     缺的是 eval 路径外部状态投影，`manufacture_external_state_for_current_with_mood`
     补投 `gummy_in_trade`；
  4. 巫恋×绮良挂件档——gold-flow 结算路径遇巫恋 E2 回退常规结算（低语按人
     +45 与龙舌兰投资倍率恢复），v3 附带单元测试锁定该行为；
  5. 新约能天使（拉特兰）——新增 trade_kind 35「同城加急单」（同房每名拉特兰
     +15%）＋蕾缪安「相伴」补认新约能天使（术语「能天使」两名成员）。
- v3 验证：`cargo fmt` + `cargo test` 全绿（334+8 通过，含 v2 回归基准）；
  release 构建 `plan --rotation 3 --no-fiammetta` 在官方 box/布局上正常。
  已知既有现象（与本批修改无关，基线复现）：**debug 构建在 Windows 下 `plan`
  命令栈溢出**（1MB 主线程栈），release 正常。
- 逐班复评 27 行中 21 行完全一致；6 行差异均为「基建内计数」技能（社群的意义、
  魔物料理、莱茵生命阵营）在孤立评估中基数变小，属预期上下文差异。
- 审查发现：plan 本身无明确错误（求解器输出）；2 项改进点——酒神以散件形态
  使用而红云组搭档（红云+Miss.Christine）同期可用（skill-9 检查 6 场景）。

## 相关项目与边界

- **[ArknightsInfraCalc-v3](https://github.com/KnightCodeSquareMatrix/ArknightsInfraCalc-v3)**——排班求解器。本工具是它的下游核验方：消费其 `plan.compute` 输出 JSON，并以 path 依赖调用其 `eval_v2_assignment_with_shift` 效率评估函数做独立复算。本工具自身不求解排班、不产出 plan；核验中发现的评估函数缺陷修在 v3 仓（见上节五文件修复记录），本仓不携带 v3 代码副本。
- **[RIIC-knowledge](https://github.com/lejciy/RIIC-knowledge)**（arknights-base-vault 对外版）——基建知识库。十四项检查规则（skill-9 排班表审查口径）与 31 个组合的效率锚点均编码自其正文，出处见 `src/kb.rs` 注释与 `data/combos.json` 各条 `kb_ref`；知识库数值变更后需人工同步 `kb.rs` 静态表与 `combos.json`，本仓不反向写回知识库。
- **infracalc-review（本仓）**——核验工具。输入 box、plan 与布局，输出审查报告；报告用于核对求解器输出与知识库口径是否一致，排班决策本身仍以 v3 输出与知识库正文为准。

## 检查能力边界

- 检查规则全部编码自知识库正文（`src/kb.rs` 注明出处）；知识库数值变更时
  需同步 `kb.rs` 静态表与 `data/combos.json`；
- 布局站级/配方以 `--layout` 文件为准；排班表不自报房间等级；
- 会客室「独占条件」类技能名单未全量编码（需模块2 快照支持），当前只查搭档缺位；
- 心情周期检查为线性班次（未做跨周期回绕的最长连续段）。
