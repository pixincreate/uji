local command = require("uji.command")
local ui = require("uji.ui")

command.builtin("thinking", "show or hide model reasoning", function()
    ui:toggle_thinking()
end)
