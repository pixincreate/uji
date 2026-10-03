local sys = require("uji.sys")

local STATE = uji.provider.STATE

local function counted(models, fails)
    local calls = { count = 0 }
    calls.load = function()
        calls.count = calls.count + 1
        sys.sleep(0.01)
        if fails and calls.count <= fails then
            error("offline", 0)
        end
        return models
    end
    return calls
end

local function add(id, models)
    uji.provider.add({
        id = id,
        name = id,
        api = uji.api.openai(),
        base_url = "http://127.0.0.1:9",
        models = models,
    })
end

local function ids(rows)
    local out = {}
    for index, row in ipairs(rows) do
        out[index] = row.id
    end
    return out
end

local function state(id)
    return uji.provider.get(id).state
end

it("loads a provider's models only once they are needed", function()
    local calls = counted({ { id = "fetched", context = 1000 } })
    add("lazy", calls.load)
    assert.equal(0, calls.count)
    assert.equal(STATE.IDLE, state("lazy"))
    assert.same({ "fetched" }, ids(uji.provider.load("lazy").models))
    assert.same({ "fetched" }, ids(uji.provider.load("lazy").models))
    assert.equal(1, calls.count)
    assert.equal(STATE.LOADED, state("lazy"))
end)

it("shares one load between everyone waiting for it", function()
    local calls = counted({ { id = "fetched" } })
    add("shared", calls.load)
    local done = sys.promise()
    uji.provider.load("shared", function()
        done:resolve()
    end)
    sys.sleep(0)
    assert.equal(STATE.LOADING, state("shared"))
    uji.provider.load("shared")
    done:await()
    assert.equal(1, calls.count)
end)

it("keeps the listed models when loading fails and loads again later", function()
    local calls = counted({ { id = "fetched" } }, 1)
    add("flaky", { "listed" })
    add("flaky", calls.load)
    local row = uji.provider.load("flaky")
    assert.same({ "listed" }, ids(row.models))
    assert.equal(STATE.FAILED, row.state)
    assert.equal("offline", row.error)
    row = uji.provider.load("flaky")
    assert.same({ "listed", "fetched" }, ids(row.models))
    assert.equal(STATE.LOADED, row.state)
    assert.is_nil(row.error)
    assert.equal(2, calls.count)
end)

it("resolves the current model again once its provider has loaded", function()
    local calls = counted({ { id = "thinker", reasoning = true, efforts = { "low", "high" } } })
    add("background", calls.load)
    local changed = sys.promise()
    uji.on("model_changed", function()
        if #uji.model.efforts() > 0 then
            changed:resolve()
        end
    end)
    uji.model.use({ provider = "background", model = "thinker" })
    assert.same({}, uji.model.efforts())
    assert.is_true(uji.task.timeout(5, function()
        changed:await()
    end))
    assert.same({ "low", "high" }, uji.model.efforts())
end)

it("rejects models that are neither a list nor a function", function()
    local ok, err = pcall(add, "broken", "not a list")
    assert.is_false(ok)
    assert.truthy(err:find("models must be a list or a function that returns one", 1, true))
end)

it("gives nothing for an unknown provider and points a load of one at the caller", function()
    assert.is_nil(uji.provider.get("nobody"))
    local ok, err = pcall(function()
        local row = uji.provider.load("nobody")
        return row
    end)
    assert.is_false(ok)
    assert.truthy(err:find("providers_spec.lua", 1, true))
    assert.truthy(err:find("no provider is registered as nobody", 1, true))
end)
