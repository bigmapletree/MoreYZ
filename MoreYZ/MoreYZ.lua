-- =============================================================
-- [[ MoreYZ - 恶魔暴君爆发监控 ]]
-- 监控召唤恶魔暴君后25秒内古尔丹之手的释放次数
-- 推算召唤恶魔数量，爆发结束后通报小队
-- =============================================================

local addonName, ns = ...

-- =============================================================
-- 常量
-- =============================================================
local TYRANT_SPELL_ID     = 265187   -- 召唤恶魔暴君
local HAND_SPELL_ID       = 105174   -- 古尔丹之手
local BURST_WINDOW        = 25       -- 爆发窗口（秒）
local HANDS_PER_DEMON     = 2        -- 每几个古尔丹之手算一个恶魔

-- =============================================================
-- 数据库
-- =============================================================
_G.MoreYZDB = _G.MoreYZDB or {}
local db

local DB_DEFAULTS = {
    reportToParty     = true,   -- 是否通报队伍
    reportAfterCombat = false,  -- true=脱战后汇总通报, false=每次爆发结束立即通报
    showLocalPrint    = true,   -- 本地聊天框输出
    reportToInstance  = true,   -- 副本队伍时使用副本频道通报（优先于小队频道）
}

local function InitDB()
    db = _G.MoreYZDB
    for k, v in pairs(DB_DEFAULTS) do
        if db[k] == nil then db[k] = v end
    end
end

-- =============================================================
-- 运行时状态
-- =============================================================
local MAX_HISTORY       = 10      -- 最多保留最近10条记录
local isTyrantActive   = false
local tyrantStartTime  = 0
local handCount        = 0
local burstIndex       = 0
local combatHistory    = {}       -- 当前战斗内的记录（临时）
local pendingReports   = {}       -- 脱战后待发消息
local burstTimer       = nil
local inCombat         = false
local inBossEncounter  = false    -- 当前是否在Boss战斗中（ENCOUNTER_START/END实时状态）
local hadBossEncounter = false    -- 本次战斗是否经历过Boss（用于脱战时判断，消费后重置）

-- =============================================================
-- 工具函数
-- =============================================================
local function Print(msg)
    print("|cff8788EE[MoreYZ]|r " .. msg)
end

--- 评价文案（根据恶魔数量，6档）
local function GetEvaluation(demons)
    if demons >= 5 then
        return "夯！"
    elseif demons == 4 then
        return "顶级"
    elseif demons == 3 then
        return "人上人"
    elseif demons == 2 then
        return "NPC"
    elseif demons == 1 then
        return "拉完了"
    else
        return "潜力股"
    end
end

--- 构造通报文本
local function BuildReportMessage(index, hands, demons)
    local eval = GetEvaluation(demons)
    return string.format("第%d次暴君：%d个古尔丹之手 → 召唤了%d个恶魔，%s",
        index, hands, demons, eval)
end

--- 发送队伍消息（自动选择频道：副本队伍优先，否则小队）
local function SendPartyMessage(msg)
    if not db.reportToParty then return end
    if not IsInGroup() then return end

    -- 副本队伍（随机副本/随机团等）优先使用 INSTANCE_CHAT
    if db.reportToInstance and IsInGroup(LE_PARTY_CATEGORY_INSTANCE) then
        SendChatMessage(msg, "INSTANCE_CHAT")
    else
        SendChatMessage(msg, "PARTY")
    end
end

--- 本地输出
local function LocalPrint(msg)
    if db.showLocalPrint then
        Print(msg)
    end
end

-- =============================================================
-- 核心逻辑
-- =============================================================

--- 结束当前爆发窗口
local function FinalizeBurst()
    if not isTyrantActive then return end
    isTyrantActive = false

    local demons = math.floor(handCount / HANDS_PER_DEMON)
    local record = {
        hands  = handCount,
        demons = demons,
        time   = date("%m/%d %H:%M"),
    }

    -- 持久化到 db.history，保留最近 MAX_HISTORY 条
    table.insert(db.history, record)
    while #db.history > MAX_HISTORY do
        table.remove(db.history, 1)
    end

    local msg = BuildReportMessage(burstIndex, handCount, demons)
    LocalPrint(msg)

    if inCombat then
        -- 战斗中结算：记录到本场战斗历史
        table.insert(combatHistory, record)
        -- 判断是否需要延迟通报
        if db.reportAfterCombat or inBossEncounter then
            table.insert(pendingReports, { index = burstIndex, hands = handCount, demons = demons })
        else
            SendPartyMessage(msg)
        end
    else
        -- 脱战后计时器到期：直接发送，不再纳入战斗汇总
        SendPartyMessage(msg)
    end

    handCount = 0
    burstTimer = nil
end

--- 开始新一轮爆发窗口
local function StartBurst()
    if isTyrantActive then
        FinalizeBurst()
    end

    burstIndex = burstIndex + 1
    handCount = 0
    isTyrantActive = true
    tyrantStartTime = GetTime()

    LocalPrint(string.format("第%d次暴君爆发开始！（%ds 监控窗口）", burstIndex, BURST_WINDOW))

    burstTimer = C_Timer.After(BURST_WINDOW, function()
        FinalizeBurst()
    end)
end

--- 记录一次古尔丹之手
local function RecordHand()
    if not isTyrantActive then return end
    handCount = handCount + 1
end

--- 脱战后发送队列（合并成一条汇总消息）
local function FlushPendingReports()
    if #pendingReports == 0 then return end

    local parts = {}
    local totalDemons = 0
    for _, rec in ipairs(pendingReports) do
        local eval = GetEvaluation(rec.demons)
        table.insert(parts, string.format("第%d次:%d手→%d魔(%s)", rec.index, rec.hands, rec.demons, eval))
        totalDemons = totalDemons + rec.demons
    end

    local summary
    if #pendingReports == 1 then
        local r = pendingReports[1]
        summary = BuildReportMessage(r.index, r.hands, r.demons)
    else
        summary = string.format("暴君汇总(%d次,共%d恶魔): %s",
            #pendingReports, totalDemons, table.concat(parts, " | "))
    end

    SendPartyMessage(summary)
    wipe(pendingReports)
end

--- 战斗结束清理
local function OnCombatEnd()
    local isBossEnd = hadBossEncounter

    -- Boss 脱战：提前结算活跃爆发
    -- 注意：此时 inCombat 仍为 true，FinalizeBurst 会正确入队
    if isTyrantActive and isBossEnd then
        if burstTimer then
            burstTimer:Cancel()
            burstTimer = nil
        end
        FinalizeBurst()
    end

    -- 本地打印战斗汇总
    if #combatHistory > 0 then
        LocalPrint("--- 本场战斗暴君汇总 ---")
        local totalDemons = 0
        for i, rec in ipairs(combatHistory) do
            local eval = GetEvaluation(rec.demons)
            LocalPrint(string.format("  第%d次: %d手 → %d恶魔 %s", i, rec.hands, rec.demons, eval))
            totalDemons = totalDemons + rec.demons
        end
        LocalPrint(string.format("  合计: %d次暴君，%d个恶魔", #combatHistory, totalDemons))
        LocalPrint("------------------------")
    end

    -- 发送待发报告
    FlushPendingReports()

    -- 始终清理本场战斗数据（活跃爆发的计时器会自行处理）
    burstIndex = 0
    wipe(combatHistory)
    wipe(pendingReports)
end

-- =============================================================
-- 事件处理
-- =============================================================
local eventFrame = CreateFrame("Frame")

eventFrame:RegisterEvent("PLAYER_LOGIN")
eventFrame:RegisterEvent("UNIT_SPELLCAST_SUCCEEDED")
eventFrame:RegisterEvent("PLAYER_REGEN_DISABLED")
eventFrame:RegisterEvent("PLAYER_REGEN_ENABLED")
eventFrame:RegisterEvent("ENCOUNTER_START")
eventFrame:RegisterEvent("ENCOUNTER_END")

eventFrame:SetScript("OnEvent", function(_, event, ...)
    if event == "PLAYER_LOGIN" then
        InitDB()
        if not db.history then db.history = {} end
        Print("已加载 - /myz 查看帮助")

    elseif event == "UNIT_SPELLCAST_SUCCEEDED" then
        local unit, _, spellID = ...
        if unit ~= "player" then return end

        if spellID == TYRANT_SPELL_ID then
            StartBurst()
        elseif spellID == HAND_SPELL_ID then
            RecordHand()
        end

    elseif event == "PLAYER_REGEN_DISABLED" then
        inCombat = true

    elseif event == "PLAYER_REGEN_ENABLED" then
        -- 先处理战斗结束（此时 inCombat 仍为 true）
        OnCombatEnd()
        -- 再重置状态
        inCombat = false
        inBossEncounter = false
        hadBossEncounter = false

    elseif event == "ENCOUNTER_START" then
        inBossEncounter = true
        hadBossEncounter = true
        LocalPrint("检测到Boss战斗，通报将延迟到脱战后发送")

    elseif event == "ENCOUNTER_END" then
        inBossEncounter = false
        -- hadBossEncounter 保留，留到 OnCombatEnd 消费
    end
end)

-- =============================================================
-- 斜杠命令
-- =============================================================
SLASH_MOREYZ1 = "/myz"
SLASH_MOREYZ2 = "/moreyz"
SlashCmdList["MOREYZ"] = function(input)
    local arg = (input or ""):trim():lower()

    if arg == "party" then
        db.reportToParty = not db.reportToParty
        Print("小队通报: " .. (db.reportToParty and "|cff00ff00开启|r" or "|cffff0000关闭|r"))

    elseif arg == "delay" then
        db.reportAfterCombat = not db.reportAfterCombat
        Print("脱战后通报: " .. (db.reportAfterCombat and "|cff00ff00开启|r（攒到脱战后发）" or "|cffff0000关闭|r（每次爆发结束立即发）"))

    elseif arg == "instance" then
        db.reportToInstance = not db.reportToInstance
        Print("副本频道通报: " .. (db.reportToInstance and "|cff00ff00开启|r（副本队伍时用副本频道）" or "|cffff0000关闭|r（始终用小队频道）"))

    elseif arg == "local" then
        db.showLocalPrint = not db.showLocalPrint
        Print("本地输出: " .. (db.showLocalPrint and "|cff00ff00开启|r" or "|cffff0000关闭|r"))

    elseif arg == "test" then
        Print("=== 模拟测试 ===")
        for i = 0, 6 do
            Print(BuildReportMessage(1, i * 2, i))
        end

    elseif arg == "history" then
        if not db.history or #db.history == 0 then
            Print("暂无暴君记录")
        else
            Print(string.format("--- 最近%d次暴君记录 ---", #db.history))
            for i, rec in ipairs(db.history) do
                local eval = GetEvaluation(rec.demons)
                local timeStr = rec.time or "未知时间"
                Print(string.format("  #%d [%s] %d手 → %d恶魔 %s", i, timeStr, rec.hands, rec.demons, eval))
            end
            Print("------------------------")
        end

    elseif arg == "clear" then
        if db.history then wipe(db.history) end
        Print("历史记录已清空")

    else
        Print("命令列表:")
        Print("  /myz party    - 开关队伍通报")
        Print("  /myz instance - 开关副本频道通报（随机副本时优先）")
        Print("  /myz delay    - 切换立即/脱战后通报")
        Print("  /myz local    - 开关本地聊天框输出")
        Print("  /myz test     - 模拟测试各档评价")
        Print("  /myz history  - 查看最近10次暴君记录")
        Print("  /myz clear    - 清空历史记录")
    end
end
