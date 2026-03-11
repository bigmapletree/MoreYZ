# MoreYZ

恶魔术暴君爆发监控插件（WoW 12.0）

监控召唤恶魔暴君后 25 秒内古尔丹之手的释放次数，推算召唤恶魔数量，自动通报队伍。

## 功能

- 自动检测恶魔暴君施放，开启 25 秒爆发监控窗口
- 统计古尔丹之手次数，计算召唤恶魔数量
- 6 档趣味评价：夯！/ 顶级 / 人上人 / NPC / 拉完了 / 潜力股
- 自动选择通报频道：副本队伍用副本频道，普通组队用小队频道
- Boss 战自动延迟到脱战后通报（绕过暴雪聊天限制）
- 保留最近 10 次暴君历史记录（跨战斗持久化）

## 安装

1. 下载本仓库
2. 将 `MoreYZ` 文件夹复制到：
   ```
   World of Warcraft/_retail_/Interface/AddOns/
   ```
3. 最终路径结构应为：
   ```
   World of Warcraft/_retail_/Interface/AddOns/MoreYZ/MoreYZ.toc
   World of Warcraft/_retail_/Interface/AddOns/MoreYZ/MoreYZ.lua
   ```
4. 重启游戏或在角色选择界面点击「插件」确认已启用

## 使用

在游戏聊天框输入以下命令：

| 命令            | 说明                               |
| --------------- | ---------------------------------- |
| `/myz`          | 查看帮助                           |
| `/myz party`    | 开关队伍通报                       |
| `/myz instance` | 开关副本频道通报（随机副本时优先） |
| `/myz delay`    | 切换立即 / 脱战后通报              |
| `/myz local`    | 开关本地聊天框输出                 |
| `/myz test`     | 模拟测试各档评价                   |
| `/myz history`  | 查看最近 10 次暴君记录             |
| `/myz clear`    | 清空历史记录                       |
