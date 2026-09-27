local app = require("uji.app")
local command = require("uji.command")
local notices = require("uji.notices")

command.builtin("compact", "summarise earlier messages to free context", function()
    if not app.agent:compact(app.agent:keep_recent_now()) then
        notices.push("nothing to compact yet")
    end
end)
