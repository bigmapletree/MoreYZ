-- 最简测试版本
local frame = CreateFrame("Frame")
frame:RegisterEvent("COMBAT_LOG_EVENT_UNFILTERED")
frame:RegisterEvent("ADDON_LOADED")

local eventCount = 0

frame:SetScript("OnEvent", function(self, event, ...)
    if event == "ADDON_LOADED" then
        local name = ...
        if name == "MoreYZ" then
            print("|cFF00FF00[MoreYZ]|r 简化测试版加载成功")
        end
        return
    end

    if event == "COMBAT_LOG_EVENT_UNFILTERED" then
        eventCount = eventCount + 1
        
        -- 获取战斗日志数据
        local timestamp, subevent, hideCaster, sourceGUID, sourceName, sourceFlags, sourceRaidFlags, destGUID, destName, destFlags, destRaidFlags, spellId, spellName = CombatLogGetCurrentEventInfo()
        
        -- 只打印前5次事件（避免刷屏）
        if eventCount <= 5 then
            print(string.format("|cFFFF00FF[MoreYZ] #%d subevent=%s source=%s spell=%s|r", 
                eventCount, 
                tostring(subevent), 
                tostring(sourceName), 
                tostring(spellName)))
        end
        
        -- 只追踪玩家的施法成功
        if subevent == "SPELL_CAST_SUCCESS" and sourceGUID == UnitGUID("player") then
            print(string.format("|cFF00FF00[MoreYZ] 你施放了: %s (ID:%s)|r", tostring(spellName), tostring(spellId)))
        end
    end
end)

SLASH_MOREYZ1 = "/myz"
SlashCmdList["MOREYZ"] = function(msg)
    print(string.format("[MoreYZ] 当前事件计数: %d", eventCount))
end
