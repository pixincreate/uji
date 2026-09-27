local sys = require("uji.sys")

for _, name in ipairs(sys.modules("uji.commands")) do
    require(name)
end
