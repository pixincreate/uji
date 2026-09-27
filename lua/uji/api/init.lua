local sys = require("uji.sys")

for _, name in ipairs(sys.modules("uji.api")) do
    require(name)
end
