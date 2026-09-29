local app = require("uji.app")
local notices = require("uji.notices")
local process = require("uji.system.process")
local task = require("uji.task")

local function argv(cmd)
    if type(cmd) == "string" then
        return { "sh", "-c", cmd }
    end
    if type(cmd) == "table" then
        if #cmd == 0 then
            error("cmd list must not be empty", 3)
        end
        local out = {}
        for index, part in ipairs(cmd) do
            out[index] = tostring(part)
        end
        return out
    end
    error("cmd must be a string or a list of strings", 3)
end

local function call(handler, ...)
    if handler then
        local ok, err = pcall(handler, ...)
        if not ok then
            notices.push("job: " .. tostring(err))
        end
    end
end

local function start(opts)
    local command = argv(opts.cmd)
    local cwd = opts.cwd or (app.session and app.session.directory)
    local job = { stopped = false }
    local proc, err = process.spawn({ argv = command, cwd = cwd, stdin = true })
    if not proc then
        task.spawn(function()
            call(opts.on_stderr, "spawn: " .. tostring(err))
            call(opts.on_exit, -1)
        end)
        return {
            send = function() end,
            close = function() end,
            stop = function() end,
        }
    end
    task.spawn(function()
        local result = process.watch(proc, opts.timeout, function(stream, line)
            call(stream == "stderr" and opts.on_stderr or opts.on_stdout, line)
        end)
        if result.timed_out then
            return call(opts.on_exit, -1, "timeout")
        end
        if job.stopped then
            return call(opts.on_exit, -1, "stopped")
        end
        call(opts.on_exit, result.code)
    end)
    local queue = task.sequence()
    return {
        send = function(text)
            local data = tostring(text)
            if data:sub(-1) ~= "\n" then
                data = data .. "\n"
            end
            queue(function()
                proc:write(data)
            end)
        end,
        close = function()
            queue(function()
                proc:close()
            end)
        end,
        stop = function()
            job.stopped = true
            proc:kill()
        end,
    }
end

uji.job = {
    start = function(opts)
        if type(opts) ~= "table" then
            error("uji.job.start needs a table", 2)
        end
        return start(opts)
    end,
}
