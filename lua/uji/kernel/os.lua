local ffi = require("ffi")

local PLATFORMS = { OSX = "macos", Linux = "linux", Windows = "windows" }

return { os = { platform = PLATFORMS[ffi.os] or "other" } }
