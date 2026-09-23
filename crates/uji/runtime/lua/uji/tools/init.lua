for _, name in ipairs({ "read_file", "edit_file", "write_file", "run_command" }) do
    uji.tool.register(name, require("uji.tools." .. name))
end
