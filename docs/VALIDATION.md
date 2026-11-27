# 验证方式

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

四组分别检查4个受限河牌、6个一般河牌、6个转牌、4个翻牌问题；独立枚举、LP/BR与固定私牌三街反向递推不使用引擎评估器作为真值。既有oracle自测2+5+4项。门槛由冻结manifest及脚本定义，质量目标、迭代/时间限制与独立数值通过分别记录。

输出在忽略的 `.artifacts/solver` 或 `.artifacts/af`，不提交原生大树/日志。Windows资源runner只代表相应环境；容量采样保留64/128深树超预算失败，1176组合简化树不等于三街深树通过。

最近实际功能验证见[状态](STATUS.md)和[历史验收](archive/2026-11-27/BASELINE-ACCEPTANCE-2026-11-27.md)。文档变更只检查引用、归档和组件一致性，不把历史结果写成重新执行。
