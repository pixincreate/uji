local sys = require("uji.sys")

local M = {}

function M.directory()
    return M.session and M.session.directory or sys.os.cwd()
end

return M
