local command = require("uji.command")
local config = require("uji.config")

command.builtin("reload", "reload config and plugins", function()
    config.reload()
end)
