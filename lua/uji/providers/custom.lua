local catalog = require("uji.catalog")

catalog.builtin({
    id = "custom",
    name = "Custom",
    wire = "openai-chat",
    base_url = "",
    auth_env = {  },
    models = {  },
})
