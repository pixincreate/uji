local sys = require("uji.sys")

for _, name in ipairs(sys.modules("uji.tools")) do
    require(name)
end
