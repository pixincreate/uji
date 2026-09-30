local call = ...

local function finish(db, ok, ...)
    if ok then
        db:exec("COMMIT")
        return ...
    end
    db:exec("ROLLBACK")
    error((...), 0)
end

return function(db, run)
    db:exec("BEGIN")
    return finish(db, call(run))
end
