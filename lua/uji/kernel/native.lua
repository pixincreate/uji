local ffi = require("ffi")

ffi.cdef(UJI_NATIVE.cdef)

local natives = {}

for _, native in ipairs(UJI_NATIVE.natives) do
    natives[native.name] = ffi.cast(native.signature, native.address)
end

return natives
