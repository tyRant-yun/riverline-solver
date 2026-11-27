# 计算依赖与采用记录

更新：2026-11-27。Rust层映射、调度与查询；CFR、手牌评估、范围、建树和原生存档复用完整上游，没有上游源码补丁。当前真实模型为一般HU河牌、受限转牌/翻牌，见[Solver状态](https://github.com/tyRant-yun/riverline-solver/blob/main/docs/STATUS.md)。

| 依赖 | 固定来源 | 原有许可 |
| --- | --- | --- |
| [postflop-solver](https://github.com/b-inary/postflop-solver/tree/9d1509fe5077d019825f833eed04b16d342dfda1) | Git `9d1509fe5077d019825f833eed04b16d342dfda1`，Cargo依赖 | AGPL-3.0-or-later；[原文](licenses/postflop-solver-AGPL-3.0.txt) |
| bincode / bincode_derive | crates.io，2.0.0-rc.3 | MIT；匹配上游旧Decode trait |
| serde / serde_json | 清单精确版本与Cargo.lock | MIT OR Apache-2.0 |
| sha2 | 0.10.9 | MIT OR Apache-2.0；用于字节哈希，不参与扑克算法 |
| 其余依赖 | Cargo.lock来源与checksum | 按对应包记录，更新lock时复核 |

固定Rust1.85.1，关闭默认features，仅开启bincode；不启用rayon、custom-alloc、zstd。保持单线程/不压缩。上游源码/编译缓存不入库；构建脚本的显式网络开关只影响子进程，不修改全局代理/Git配置。

依赖许可与原文保留，进程隔离不改变依赖许可。本文不重新指定原创代码许可证或宣称可闭源发布。此前受限单下注阶段的采用记录见Solver仓库历史归档；本轮只更新文档能力描述，不改变依赖、算法或许可原文。
