# 独立仓库基线验收

2026-11-27，在 `E:\riverline-solver` 使用独立Node依赖、Rust构建目录和Python oracle环境进行验证。

- `npm install --ignore-scripts` 后生成锁文件；`npm run contracts:check`：12份引擎schema与TS来源一致。
- `npm run solver:check`：固定工具链格式/clippy通过，14个Rust测试通过。
- `npm run solver:build`：新目录release构建成功。
- `benchmark:solver:small/tree/turn/flop` 全部执行：4个受限河牌、6个一般河牌、6个转牌、4个翻牌独立数值问题通过；原oracle自测2+5+4项通过。

计算源码与总产品7e0f9af的提取清单保持一致，见 `provenance/baseline.json`。没有重写CFR或放宽数值门。产品业务合同、Node工作区服务和浏览器应用没有搬入本仓库。

现有支持边界仍是给定范围的HU子局面；翻前、多人、任意完整整手和宽范围三街均未因此获验收。部署、二进制发布、生产内存隔离和平台上传未实施。
