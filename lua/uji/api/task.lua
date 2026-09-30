local sys = require("uji.sys")
local task = require("uji.task")

uji.task = {
    spawn = task.spawn,
    race = task.race,
    timeout = task.timeout,
    on_error = sys.task.on_error,
}
