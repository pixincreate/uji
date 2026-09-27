local command = require("uji.command")
local ui = require("uji.ui")

command.builtin("quit", "leave uji", function()
    ui:quit()
end)
