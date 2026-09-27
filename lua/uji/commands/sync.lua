local command = require("uji.command")
local config = require("uji.config")
local packs = require("uji.packs")

command.builtin("sync", "update installed packs", function()
    packs.update()
    config.reload()
end)
