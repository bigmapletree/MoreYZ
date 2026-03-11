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
    reportToParty     = true,   -- 是否通报小队
    reportAfterCombat = false,  -- true=脱战后汇总通报, false=每次爆发结束立即通报
    showLocalPrint    = true,   -- 本地聊天框输出
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
local isTyrantActive   = false
local tyrantStartTime  = 0
local handCount        = 0
local burstIndex       = 0
local burstHistory     = {}       -- { {hands=N, demons=M}, ... }
local pendingReports   = {}       -- 脱战后待发消息
local burstTimer       = nil
local inCombat         = false

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

--- 发送小队消息
local function SendPartyMessage(msg)
    if not db.reportToParty then return end
    if IsInGroup() then
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
    table.insert(burstHistory, { hands = handCount, demons = demons })

    local msg = BuildReportMessage(burstIndex, handCount, demons)
    LocalPrint(msg)

    if db.reportAfterCombat then
        table.insert(pendingReports, msg)
    else
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

--- 脱战后发送队列
local function FlushPendingReports()
    if #pendingReports == 0 then return end
    for _, msg in ipairs(pendingReports) do
        SendPartyMessage(msg)
    end
    wipe(pendingReports)
end

--- 战斗结束清理
local function OnCombatEnd()
    if isTyrantActive then
        if burstTimer then
            burstTimer:Cancel()
            burstTimer = nil
        end
        FinalizeBurst()
    end

    if #burstHistory > 0 then
        LocalPrint("--- 本场战斗暴君汇总 ---")
        local totalDemons = 0
        for i, rec in ipairs(burstHistory) do
            local eval = GetEvaluation(rec.demons)
            LocalPrint(string.format("  第%d次: %d手 → %d恶魔 %s", i, rec.hands, rec.demons, eval))
            totalDemons = totalDemons + rec.demons
        end
        LocalPrint(string.format("  合计: %d次暴君，%d个恶魔", #burstHistory, totalDemons))
        LocalPrint("------------------------")
    end

    FlushPendingReports()

    burstIndex = 0
    wipe(burstHistory)
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

eventFrame:SetScript("OnEvent", function(_, event, ...)
    if event == "PLAYER_LOGIN" then
        InitDB()
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
        inCombat = false
        OnCombatEnd()
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

    elseif arg == "local" then
        db.showLocalPrint = not db.showLocalPrint
        Print("本地输出: " .. (db.showLocalPrint and "|cff00ff00开启|r" or "|cffff0000关闭|r"))

    elseif arg == "test" then
        Print("=== 模拟测试 ===")
        for i = 0, 6 do
            Print(BuildReportMessage(1, i * 2, i))
        end

    elseif arg == "history" then
        if #burstHistory == 0 then
            Print("本场战斗暂无暴君记录")
        else
            for i, rec in ipairs(burstHistory) do
                Print(BuildReportMessage(i, rec.hands, rec.demons))
            end
        end

    else
        Print("命令列表:")
        Print("  /myz party   - 开关小队通报")
        Print("  /myz delay   - 切换立即/脱战后通报")
        Print("  /myz local   - 开关本地聊天框输出")
        Print("  /myz test    - 模拟测试各档评价")
        Print("  /myz history - 查看本场战斗暴君记录")
    end
end
