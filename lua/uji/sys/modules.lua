local exports = require("uji.sys.exports")
local ffi = require("ffi")
local native = require("uji.sys.native")

local EXTENSION = ffi.os == "OSX" and ".dylib" or ffi.os == "Windows" and ".dll" or ".so"

ffi.cdef([[
const char *uji_cdef(void);
const char *uji_lua(void);
void uji_module_init(const uji_host *host);
const uji_manifest *uji_module_manifest(void);
]])

local M = { paths = {}, loaded = {} }

local function symbol(library, name)
    local ok, found = pcall(function()
        return library[name]
    end)
    return ok and found or nil
end

local function exists(file)
    local handle = io.open(file, "rb")
    if handle then
        handle:close()
    end
    return handle ~= nil
end

function M.find(name)
    for _, directory in ipairs(M.paths) do
        local file = directory .. "/" .. name .. EXTENSION
        if exists(file) then
            return file
        end
    end
end

local function mount(manifest)
    local cdef = ffi.string(manifest.cdef)
    if cdef ~= "" then
        ffi.cdef(cdef)
    end
    return exports.build(native.read(manifest)).functions
end

local function handwritten(library, name)
    local declarations = symbol(library, "uji_cdef")
    if declarations then
        ffi.cdef(ffi.string(declarations()))
    end
    local wrapper = symbol(library, "uji_lua")
    if not wrapper then
        return library
    end
    return assert(load(ffi.string(wrapper()), "=" .. name))(library, require("uji.sys"))
end

function M.load(file, name)
    local library = ffi.load(file)
    local init = symbol(library, "uji_module_init")
    if init then
        init(native.kernel.kernel_host())
    end
    local manifest = symbol(library, "uji_module_manifest")
    local module = manifest and mount(manifest()) or handwritten(library, name)
    M.loaded[name] = { file = file, library = library }
    return module
end

function M.searcher(name)
    local file = M.find(name)
    if not file then
        return "\n\tno native module '" .. name .. EXTENSION .. "' in the modules folders"
    end
    return function()
        return M.load(file, name)
    end, file
end

return M
