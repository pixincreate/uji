local answer = require("uji.kernel.answer")
local exports = require("uji.kernel.exports")
local json = require("uji.kernel.json").json

local Db = exports:class("db")

local function check(ok, err)
    if not ok then
        error(err, 3)
    end
end

local function finish(db, ok, ...)
    if ok then
        check(db:exec("COMMIT"))
        return ...
    end
    check(db:exec("ROLLBACK"))
    error((...), 0)
end

function Db:transaction(run)
    check(self:exec("BEGIN"))
    return finish(self, pcall(run))
end

function Db:close()
    answer.release(self.handle)
    self.handle = nil
end

return { db = { null = json.null } }
