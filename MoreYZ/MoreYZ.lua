local f = CreateFrame("Frame")
local combatLogBuffer = {}
local MAX_PRINT_LINES = 10  -- 聊天窗口只打印前10行
local debugEventCount = 0   -- 调试：统计事件触发次数

f:RegisterEvent("ADDON_LOADED")
f:RegisterEvent("COMBAT_LOG_EVENT_UNFILTERED")
f:RegisterEvent("PLAYER_REGEN_ENABLED")
f:RegisterEvent("PLAYER_REGEN_DISABLED")

f:SetScript("OnEvent", function(self, event, ...)
    if event == "ADDON_LOADED" then
        local name = ...
        if name == "MoreYZ" then
            MoreYZDB = MoreYZDB or { enabled = true }
            print("|cFF00FF00[MoreYZ]|r 加载成功。战斗后将自动分析解密数据。")
        end

    elseif event == "PLAYER_REGEN_DISABLED" then
        -- 战斗开始，清空上一轮缓存
        combatLogBuffer = {}
        debugEventCount = 0
        print("|cFFFF0000[MoreYZ DEBUG] 进入战斗，缓存已清空|r")

    elseif event == "COMBAT_LOG_EVENT_UNFILTERED" then
        -- 调试：统计事件触发次数
        debugEventCount = debugEventCount + 1
        
        -- 尝试获取数据
        local info = { CombatLogGetCurrentEventInfo() }
        table.insert(combatLogBuffer, info)
        
        -- 每10次打印一次调试信息（避免刷屏）
        if debugEventCount <= 3 then
            print(string.format("|cFFFF00FF[MoreYZ DEBUG] CLEU事件 #%d, info长度=%d|r", debugEventCount, #info))
        end

    elseif event == "PLAYER_REGEN_ENABLED" then
        -- 【脱战解密阶段】
        print(string.format("|cFFFF0000[MoreYZ DEBUG] 脱战! enabled=%s, bufferSize=%d, 事件触发次数=%d|r", tostring(MoreYZDB.enabled), #combatLogBuffer, debugEventCount))
        
        if MoreYZDB.enabled and #combatLogBuffer > 0 then
            local totalLines = #combatLogBuffer
            print(string.format("|cFFFFFF00[MoreYZ] 战斗已结束，共 %d 条日志，打印前 %d 条：|r", totalLines, MAX_PRINT_LINES))
            
            -- 初始化日志存储
            MoreYZDB.lastCombatLog = {}
            
            for i, rawData in ipairs(combatLogBuffer) do
                -- 从 rawData 表格中解包
                -- rawData[2] 是 subevent, rawData[5] 是 sourceName, rawData[13] 是 spellName
                local subevent = rawData[2]
                local sourceName = rawData[5] or "未知"
                local spellName = rawData[13] or "普通攻击/其他"
                local spellId = rawData[12] or 0
                
                local logLine = string.format("%d. [%s] %s -> %s (ID:%s)", i, subevent, sourceName, spellName, tostring(spellId))
                
                -- 只打印前 MAX_PRINT_LINES 行
                if i <= MAX_PRINT_LINES then
                    print(logLine)
                end
                
                -- 但存储所有日志到 SavedVariables
                table.insert(MoreYZDB.lastCombatLog, logLine)
            end
            
            print(string.format("|cFF00FF00[MoreYZ] 全部 %d 条日志已保存到 SavedVariables，/reload 后可查看文件|r", totalLines))
            
            -- 清理内存
            combatLogBuffer = {}
        end
    end
end)

-- 指令部分保持不变
SLASH_MOREYZ1 = "/myz"
SlashCmdList["MOREYZ"] = function(msg)
    if msg == "on" then MoreYZDB.enabled = true print("MoreYZ 打印已开启")
    elseif msg == "off" then MoreYZDB.enabled = false print("MoreYZ 打印已关闭")
    else print("用法: /myz on|off") end
end
