local sys = require("uji.sys")

for _, name in ipairs(sys.modules("uji.wires")) do
    require(name)
end
