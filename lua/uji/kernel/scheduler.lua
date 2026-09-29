local natives = require("uji.kernel.native")

local poll, take, forget = natives.kernel_poll, natives.kernel_take, natives.kernel_cancel
local stopping, report, timer = natives.kernel_stopping, natives.kernel_report, natives.kernel_sleep

local SUSPENDED = {}
local OUTSIDE = "this call has to run inside a task"
local NESTED = "an async call cannot wait inside a coroutine of its own"
local STRAY = "a task yielded without waiting on anything"

local M = {}

local queue, first, last = {}, 1, 0
local waiting = {}
local alive = 0
local current
local handler
local released = false

local function pack(...)
    return { n = select("#", ...), ... }
end

local function schedule(task, values)
    last = last + 1
    queue[last] = { task = task, values = values }
end

local function running(level)
    local task = current
    if not task then
        error(OUTSIDE, level + 1)
    end
    if coroutine.running() ~= task.thread then
        error(NESTED, level + 1)
    end
    return task
end

local function fail(message)
    if handler and pcall(handler, message) then
        return
    end
    report(message, #message)
end

local Task = {}
Task.__index = Task

local function settle(task)
    task.done = true
    task.thread = nil
    alive = alive - 1
    if task.parent and task.parent.children then
        task.parent.children[task] = nil
    end
end

function Task:cancel()
    if self.done then
        return
    end
    settle(self)
    local children = self.children
    self.children = nil
    for child in pairs(children or {}) do
        child:cancel()
    end
    if self.token then
        waiting[self.token] = nil
        forget(self.token)
        self.token = nil
    end
    released = true
end

local function step(task, values)
    if task.done then
        return
    end
    local thread = task.thread
    current = task
    local ok, err = coroutine.resume(thread, unpack(values, 1, values.n))
    current = nil
    if task.done then
        if task.token then
            waiting[task.token] = nil
            forget(task.token)
            task.token = nil
        end
        return
    end
    if not ok then
        settle(task)
        return fail(tostring(err))
    end
    if coroutine.status(thread) == "dead" then
        settle(task)
    elseif err ~= SUSPENDED then
        settle(task)
        fail(STRAY)
    end
end

function M.spawn(fn, ...)
    if type(fn) ~= "function" then
        error("spawn needs a function, not a " .. type(fn), 2)
    end
    local task = setmetatable({ thread = coroutine.create(fn) }, Task)
    alive = alive + 1
    schedule(task, pack(...))
    return task
end

local function child(parent, fn)
    local task = M.spawn(fn)
    task.parent = parent
    parent.children = parent.children or {}
    parent.children[task] = true
    return task
end

function M.await(token)
    local task = running(3)
    task.token = token
    waiting[token] = task
    return coroutine.yield(SUSPENDED)
end

local Promise = {}
Promise.__index = Promise

function M.promise()
    return setmetatable({ settled = false, waiters = {} }, Promise)
end

function Promise:resolve(...)
    if self.settled then
        return false
    end
    self.settled = true
    self.values = pack(...)
    local waiters = self.waiters
    self.waiters = nil
    for _, task in ipairs(waiters) do
        schedule(task, self.values)
    end
    return true
end

function Promise:await()
    if not self.settled then
        local task = running(2)
        self.waiters[#self.waiters + 1] = task
        coroutine.yield(SUSPENDED)
    end
    return unpack(self.values, 1, self.values.n)
end

function M.sleep(seconds)
    if type(seconds) ~= "number" or seconds < 0 or seconds ~= seconds or seconds == math.huge then
        error("sleep needs a number of seconds", 2)
    end
    local task = running(2)
    if seconds == 0 then
        schedule(task, pack())
        coroutine.yield(SUSPENDED)
        return
    end
    local answer = M.await(tonumber(timer(seconds)))
    answer.free(answer)
end

function M.race(...)
    local count = select("#", ...)
    if count == 0 then
        error("race needs at least one function", 2)
    end
    local parent = running(2)
    local finished = M.promise()
    local racers = {}
    for index = 1, count do
        local fn = select(index, ...)
        racers[index] = child(parent, function()
            finished:resolve(index, pack(pcall(fn)))
        end)
    end
    local index, outcome = finished:await()
    for _, racer in ipairs(racers) do
        racer:cancel()
    end
    if not outcome[1] then
        error(outcome[2], 0)
    end
    return index, unpack(outcome, 2, outcome.n)
end

local function timed(index, ...)
    if index == 1 then
        return true, ...
    end
    return false
end

function M.timeout(seconds, fn)
    if type(seconds) ~= "number" or seconds < 0 or seconds ~= seconds then
        error("timeout needs a number of seconds", 2)
    end
    return timed(M.race(fn, function()
        M.sleep(seconds)
    end))
end

function M.on_error(fn)
    handler = fn
end

local function deliver(token)
    local task = waiting[token]
    waiting[token] = nil
    local answer = take(token)
    if task and task.token == token and not task.done then
        task.token = nil
        schedule(task, pack(answer))
    elseif answer ~= nil then
        answer.free(answer)
    end
end

local function receive(block)
    local token = tonumber(poll(0, not block))
    while token ~= 0 do
        deliver(token)
        token = tonumber(poll(0, true))
    end
end

local function drain()
    local stop = last
    while first <= stop do
        local entry = queue[first]
        queue[first] = nil
        first = first + 1
        step(entry.task, entry.values)
    end
    if first > last then
        queue, first, last = {}, 1, 0
    end
end

function M.run(main, ...)
    M.spawn(main, ...)
    while true do
        drain()
        if alive == 0 or stopping() then
            return
        end
        if released then
            released = false
            collectgarbage()
        end
        receive(first > last)
    end
end

return M
