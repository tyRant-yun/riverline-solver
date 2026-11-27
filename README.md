# Riverline Solver

独立真实Solver仓库，复用固定 `postflop-solver` 引擎，负责协议、预检、计算、取消和原生有界查询。支持一般HU河牌、受限转牌128组合/方和翻牌16组合/方；详见[当前状态](docs/STATUS.md)。

总产品：[poker_coach](https://github.com/tyRant-yun/poker_coach)；策略：[riverline-bot](https://github.com/tyRant-yun/riverline-bot)。产品保留业务作业/权限/数据权威，通过固定提交消费组件快照。

## 构建与检查

需要Node24、固定Rust1.85.1；Windows需要MSVC x64 C++工具。

```powershell
npm ci
npm run solver:build
npm run solver:check
npm run contracts:check
```

独立数值基准还需Python oracle环境，见[验证方式](docs/VALIDATION.md)。最近独立基线通过14个Rust、12份schema和20个数值问题；这不代表翻前、多人或通用全街能力。

- [当前能力/目录](docs/STATUS.md) · [开发队列](docs/DEVELOPMENT.md)
- [依赖与许可](engines/postflop/THIRD_PARTY.md) · [来源](provenance/baseline.json)
- [历史基线与旧入口](docs/archive/README.md) · [文档索引](docs/README.md)

保持单线程、依赖/工具链锁及旧原生解兼容，不把整树导出到产品快照或Agent上下文。本轮仅整合文档，算法和协议未变。
