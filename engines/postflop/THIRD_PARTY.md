# 计算依赖与采用记录

2026-11-18；本项目新增 Rust 外壳只映射/调度/查询，CFR、手牌评估、范围、建树和原生存档完整使用上游。

| 项目 | 固定来源 / 实际方式 | 许可与限制 |
|---|---|---|
| postflop-solver | https://github.com/b-inary/postflop-solver ，Git rev `9d1509fe5077d019825f833eed04b16d342dfda1`；Cargo 依赖，无源码补丁 | AGPL-3.0-or-later；原文见 `licenses/postflop-solver-AGPL-3.0.txt`；本轮只独立验证受限两人河牌模型 |
| bincode / bincode_derive | crates.io，均锁 `2.0.0-rc.3` | MIT；与上游使用的旧 Decode trait 匹配，不能按宽松版本自动更新到 2.0.1 |
| serde / serde_json | crates.io，清单精确版本及 Cargo.lock 校验和 | MIT OR Apache-2.0；仅用于本项目协议/输入输出 |
| sha2 | crates.io，锁定 0.10.9 | MIT OR Apache-2.0；R2-A 校验输入文件字节哈希，不参与扑克算法 |
| 其余构建/间接依赖 | `Cargo.lock` 完整枚举版本、来源与 checksum；通过 Cargo metadata 核对包内许可 | 采用报告保存实际 metadata 摘要；更新 lock 时复核新增依赖 |

引擎关闭默认 features，显式开启 bincode；不启用 rayon、custom-alloc、zstd。单线程、不压缩作为首批基准。Rust 1.99 原生构建遇到旧上游危险隐式引用 lint；采用固定旧工具链复验，不改计算源码，不用升级引擎消除问题。实际通过工具链和质量证据以验证报告为准。

上游源码通过 Git 取得，源码副本和编译缓存放 `.artifacts` / `target`，不提交。受限网络可按脚本的显式环境开关使用 SSH 拉取同一 HTTPS 依赖来源，不改变 commit，也不修改全局 Git 配置。

依赖许可并不因进程隔离而消失。当前记录本地采用事实，不声明整个产品可闭源分发；后续实际分发/服务须保留通知及履行相应源码义务。包内锁与源码地址允许复现，但不替代必要的依赖归档。
