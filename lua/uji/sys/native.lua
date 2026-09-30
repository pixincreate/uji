local ffi = require("ffi")

ffi.cdef(UJI_NATIVE.cdef)

local M = {}

function M.read(manifest)
    local natives, wrappers = {}, {}
    for index = 0, tonumber(manifest.native_count) - 1 do
        local native = manifest.natives[index]
        natives[ffi.string(native.name)] = ffi.cast(ffi.string(native.signature), native.address)
    end
    for index = 0, tonumber(manifest.wrapper_count) - 1 do
        local wrapper = manifest.wrappers[index]
        wrappers[index + 1] = { place = ffi.string(wrapper.place), source = ffi.string(wrapper.source) }
    end
    return natives, wrappers
end

M.kernel, M.wrappers = M.read(ffi.cast("const uji_manifest *", UJI_NATIVE.manifest))

return M
