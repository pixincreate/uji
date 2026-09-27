local sys = require("uji.sys")

for _, name in ipairs(sys.modules("uji.providers")) do
    require(name)
end
