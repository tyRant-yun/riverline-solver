# Solver当前状态

更新：2026-11-27。本轮整理文档，原生源码/协议/输入与算法不变。

| 能力 | 当前范围 |
| --- | --- |
| HU河牌 | schema1受限模型及schema2一般显式离散树，支持原生分页 |
| HU转牌 | schema3兼容16组合与schema4分页128组合/方；每街每方一个固定下注、无再加注 |
| HU翻牌 | schema5，最多16组合/方，三街单尺寸/无再加注与两次机会节点 |
| 计算与读取 | 固定规格、预检、真实worker、取消/中断、原生制品校验、有界查询 |
| 未支持 | 通用翻前、多人均衡、任意完整整手GTO、宽范围三街和产品实验发布 |

给定范围模型不推断真实对手范围，无抽水/bunching/产品锁定。组合数只是模型上限，仍需预检与预算；单个成功小问题不证明更大模型。

`engines/postflop`为Rust薄适配/测试；`contracts/solver`为引擎schema/黄金输入，产品 `business.schema.json`不在本仓；`benchmarks/solver`为独立oracle和冻结问题；`scripts`为构建与采样入口。

固定上游 `9d1509fe5077d019825f833eed04b16d342dfda1`，Rust1.85.1、bincode2.0.0-rc.3，单线程/不压缩。新增依赖或profile需新验证。

独立基线 `6569a01`：构建、14 Rust、12 schema及10河牌/6转牌/4翻牌数值门通过；[原始记录](archive/2026-11-27/BASELINE-ACCEPTANCE-2026-11-27.md)。本轮未重跑功能测试。Windows实测不代表Linux部署，进程隔离也不代表生产硬内存隔离或checkpoint续算。
