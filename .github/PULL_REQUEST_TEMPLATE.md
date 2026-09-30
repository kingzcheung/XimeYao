<!--
标题格式（conventional commits）：<type>(<scope>): <一句话说明>
示例：feat(setup): 词典管理页支持导出/导入；fix(tsf): 修复 Shift 单击误触发中英切换
-->

## 变更说明

<!-- 这个 PR 做了什么？为什么做？（背景 / 动机 / 方案要点，评审者第一眼看这里） -->

## 变更类型

- [ ] 新功能（feat）
- [ ] 缺陷修复（fix）
- [ ] 重构（refactor，不改行为）
- [ ] 性能优化（perf）
- [ ] 文档（docs）
- [ ] 构建 / CI / 打包（build / ci）
- [ ] 其他：

## 测试与验证

<!-- CI 只做 release 构建与打包，测试和实机验证请在本地完成后勾选 -->

- [ ] `cargo build --quiet` 零错误、零新增告警
- [ ] `cargo test` 通过（涉及 crate：<!-- 如 winxime-server / xime-config -->）
- [ ] 实机验证 `.\rebuild.ps1`（TSF 输入 / 托盘 / 候选栏 / 设置 UI 改动必填）
  - 验证步骤与结果：

## 硬性规则自查

<!-- 对齐 AGENTS.md，逐项确认 -->

- [ ] 代码无 `unwrap()` / `expect()`
- [ ] 未修改 `librime/` 目录（只读）
- [ ] 未顺手改动无关功能（每次只做一个功能点）
- [ ] 已更新 `PROGRESS.md`（涉及功能点时）

## UI 变更

<!-- 设置程序 / 候选栏 / 托盘有界面改动时，贴改动前后截图；无则删除本节 -->

| 改动前 | 改动后 |
| --- | --- |
| | |

## 关联

<!-- 关联 issue 用 Closes/Fixes #编号；无则删除本节 -->
