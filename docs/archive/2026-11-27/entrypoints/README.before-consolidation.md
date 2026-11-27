# Riverline Solver

Riverline 的独立真实Solver仓库。总产品：[poker_coach](https://github.com/tyRant-yun/poker_coach)；对战策略：[riverline-bot](https://github.com/tyRant-yun/riverline-bot)。

从总产品 `7e0f9af` 提取已有实现，不重写求解算法；逐文件来源见 `provenance/baseline.json`。原仓库保留完整Git历史，新仓库以有来源指纹的独立基线起步。产品目前固定消费兼容快照；UI、业务队列、工作区和权限权威继续由产品维护。

## 当前能力

固定上游 `postflop-solver` 提交 `9d1509fe5077d019825f833eed04b16d342dfda1`，固定Rust1.85.1、单线程、bincode rc.3。一般HU河牌、受限转牌最多128组合/方、受限翻牌最多16组合/方；转牌/翻牌每街每方一个固定下注尺寸，无再加注、无抽水。没有通用翻前、多人均衡或完整整手GTO能力。数值基准和实际支持边界独立于“编译成功”。

- `engines/postflop/`：Rust薄适配、worker、原生存档/分页、测试及原始许可。
- `contracts/solver/`：引擎协议/输入/节点schema1–5及黄金输入；不包含产品 `business.schema.json`。
- `benchmarks/solver/`：独立牌力枚举、LP/BR、机会节点和三街反向递推基准。
- `scripts/`：本机编译/检查/独立数值基准入口。

## 构建与验证

```powershell
npm ci
npm run solver:build
npm run solver:check
npm run contracts:check
py -m venv .artifacts/solver/oracle-venv
.artifacts/solver/oracle-venv/Scripts/python.exe -m pip install -r benchmarks/solver/requirements.txt
npm run benchmark:solver:small
npm run benchmark:solver:tree
npm run benchmark:solver:turn
npm run benchmark:solver:flop
```

Windows需要MSVC C++构建工具；脚本继承固定工具链。现有Windows资源采样不等于Linux部署验收。完整求解树不能进入产品快照或Agent上下文；只读取需要的节点/组合页。取消、预算结束、失败和质量达标不得混写。

第三方许可见 `engines/postflop/THIRD_PARTY.md` 和 `engines/postflop/licenses/`，原文保留。本次仅建立私有源码仓库，没有发布二进制或改变许可。旧产品Python L2/缓存只可作为对照候选，不能替换已有独立数值门；详情见总产品拆仓分析。
