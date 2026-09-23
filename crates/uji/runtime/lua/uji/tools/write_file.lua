local field = require("uji.tools.field")

local function line_count(text)
    local _, newlines = text:gsub("\n", "")
    if text ~= "" and text:sub(-1) ~= "\n" then
        return newlines + 1
    end
    return newlines
end

return {
    description = "Write a file from scratch, creating parent directories as needed. This replaces the entire "
        .. "file, so use it for new files only. To change an existing file use `edit_file` instead - "
        .. "overwriting loses everything you did not include.",
    parameters = {
        type = "object",
        properties = {
            path = {
                type = "string",
                description = "Path to write, absolute or relative to the working directory.",
            },
            content = {
                type = "string",
                description = "Complete contents of the file.",
            },
        },
        required = { "path", "content" },
        additionalProperties = false,
    },
    subject = function(args)
        return field.text(args, "path")
    end,
    policy = "ask",
    defer = true,
    run = function(args, done)
        local missing = field.missing(args, "path")
        if missing then
            return done(missing)
        end
        local path = field.text(args, "path")
        local content = field.text(args, "content")
        uji.fs.write(path, content, function(written, err)
            if not written then
                return done("error: " .. err)
            end
            local verb = written.created and "created" or "overwrote"
            done(string.format("%s %s (%d lines)", verb, path, line_count(content)))
        end)
    end,
}
