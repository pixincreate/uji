uji.command.add("models", {
    desc = "pick the default model",
    handler = function()
        local current = uji.model.current().provider
        for _, provider in ipairs(uji.provider.list()) do
            if provider.id == current and provider.discovery then
                local _, failure = uji.provider.refresh(current)
                if failure then
                    uji.notify("model discovery failed: " .. failure.message)
                    return
                end
            end
        end
        local available = {}
        for _, provider in ipairs(uji.provider.list()) do
            if #provider.models > 0 and (provider.id == current or uji.auth.authenticated(provider.id)) then
                available[#available + 1] = provider
            end
        end
        if #available == 0 then
            uji.notify("Please run /login to configure a provider")
            return
        end
        local qualify = #available > 1
        local items, choices = {}, {}
        for _, provider in ipairs(available) do
            for _, entry in ipairs(provider.models) do
                local label = qualify and provider.name .. " · " .. entry.id or entry.id
                choices[label] = { provider = provider.id, model = entry.id }
                items[#items + 1] = label
            end
        end
        local title = qualify and "Models" or available[1].name .. " models"
        local choice = uji.ui.select({ title = title, items = items })
        local picked = choice and choices[choice]
        if picked then
            uji.model.use(picked)
        end
    end,
})
