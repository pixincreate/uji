local sandbox = require("support.sandbox")

it("lists stored sessions and protects the open conversation from deletion", function()
    local app = require("uji.core.app")
    require("uji.api.session")
    local session = uji.session
    local store = require("uji.core.store")(sandbox.work .. "/session-api.db")
    local root = store:create_session("first")
    local child = store:create_session("child", root.id)
    local other = store:create_session("other")

    app.store = store
    app.session = child

    local rows = session.list()
    assert.equal(2, #rows)
    local seen = {}
    for _, row in ipairs(rows) do
        seen[row.id] = row
    end
    assert.equal("first", seen[root.id].title)
    assert.equal("other", seen[other.id].title)
    assert.is_nil(seen[child.id])
    assert.is_true(type(seen[root.id].updated) == "number")

    local ok, err = session.delete(child.id)
    assert.is_nil(ok)
    assert.equal("the open session cannot be deleted", err)
    ok, err = session.delete(root.id)
    assert.is_nil(ok)
    assert.equal("the open session cannot be deleted", err)
    assert.is_true(session.delete(other.id))
    assert.equal(1, #session.list())
    assert.is_true(not pcall(session.delete, "not-an-id"))
end)
