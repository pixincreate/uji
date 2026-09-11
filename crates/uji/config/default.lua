-- Default uji UI, configured via windows + buffers (nvim-style).
--
-- Config lives in ~/.config/uji (override with UJI_CONFIG_DIR):
--   init.lua        entry point, this file when you have none
--   lua/            module root; require("foo.bar") finds lua/foo/bar.lua
--                   or lua/foo/bar/init.lua
--   plugin/         *.lua here is sourced automatically after init.lua,
--                   sorted by name; number prefixes control order
--
-- Packs are extra directories with the same lua/ + plugin/ layout. Declaring
-- one clones it into ~/.local/share/uji/site if missing, then makes its
-- modules requirable and its plugin/ files auto-source:
--
--   uji.pack.add({
--     "user/repo",                      -- github shorthand
--     { "user/repo", tag = "v1.2" },    -- or branch = / commit =
--     { url = "https://git.sr.ht/~x/y" },
--     { dir = "~/code/my-plugin" },     -- local, never cloned
--   })
--   uji.pack.list()                     -- every root being searched
--
-- /sync updates installed packs and reloads. Versions are pinned in
-- ~/.local/share/uji/uji-lock.json.
--
-- SECURITY: a pack is arbitrary code from the internet, executed on the next
-- start. Read what you install.
--
-- BREAKING: lua/plugins/*.lua no longer auto-sources. Move those files to
-- plugin/ (lua/ is for require() only).

uji.ui.open_win({ view = "messages", split = "top", size = "fill", wrap = true })
local status = uji.ui.open_win({ split = "bottom", size = 1 })
uji.ui.open_win({ view = "input", split = "bottom", size = 3, border = "horizontal" })
local activity = uji.ui.open_win({ split = "bottom", size = 0, padding = 1 })

uji.ui.configure({
    input = { cursor_blink = true },
    suggest = { enabled = true, max_height = 5 },
    waiting = {
        loader = {
            frames = { "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏" },
            interval_ms = 80,
        },
    },
})

uji.on("tool_call", function(event)
    if event.name == "run_command" then
        local cmd = event.arguments.command or ""
        if string.match(cmd, "^rm %-rf") then
            return { deny = "Refusing to run rm -rf" }
        end
        return { ask = true }
    end
    -- reads and writes fall through to the default policy (allow / ask)
    return nil
end)

local waiting_text = "Working"

local function shorten(path)
    local home = os.getenv("HOME")
    if home and home ~= "" and path:sub(1, #home) == home then
        return "~" .. path:sub(#home + 1)
    end
    return path
end

local DIM = "#4a4a4a"
local MUTED = "#808080"

local function render_status()
    local spans = {}
    local function part(text, color)
        if #spans > 0 then
            spans[#spans + 1] = { text = "  ·  ", color = DIM }
        end
        spans[#spans + 1] = { text = text, color = color }
    end

    local dir = shorten(uji.session.info().directory or "")
    if dir ~= "" then
        part(dir, "cyan")
    end

    local provider = uji.status.provider()
    if provider then
        part(provider .. "/" .. uji.status.model(), MUTED)
    end

    local turns = 0
    for _, message in ipairs(uji.session.messages()) do
        if message.type == "user" then
            turns = turns + 1
        end
    end
    if turns > 0 then
        part(turns .. (turns == 1 and " turn" or " turns"), MUTED)
    end

    if #spans == 0 then
        uji.ui.clear(status)
    else
        table.insert(spans, 1, { text = " ", color = DIM })
        uji.ui.set_lines(status, { spans })
    end
end

local function render_activity()
    if uji.status.state() == "working" then
        uji.ui.set_size(activity, 3)
        local elapsed = math.floor(uji.status.elapsed() or 0)
        uji.ui.set_lines(activity, {
            {
                { text = uji.status.loader_frame() .. " ", color = "cyan", bold = true },
                { text = waiting_text .. " (" .. elapsed .. "s)", color = "#808080" },
            },
        })
    else
        uji.ui.set_size(activity, 0)
    end
end

uji.on("status_changed", function()
    render_status()
    render_activity()
end)
uji.on("tick", render_activity)
uji.on("MessageAppended", render_status)
render_status()

-- Keybindings. Every key is remappable per mode: normal, suggest, select,
-- prompt, confirm. A binding is either a builtin action name, a slash command
-- via { command = "models" }, or nil to unbind the key entirely.
--
-- Actions: quit, interrupt, submit, clear_input, backspace, cursor_left,
-- cursor_right, cursor_start, cursor_end, scroll_up, scroll_down, page_up,
-- page_down, scroll_top, scroll_bottom, modal_up, modal_down, modal_accept,
-- modal_cancel, suggest_complete, confirm_allow, confirm_deny, confirm_toggle,
-- nothing.
--
-- uji.keymap.set("normal", "<C-p>", { command = "models" })
-- uji.keymap.set("normal", "<C-u>", "clear_input")
-- uji.keymap.set("normal", "<C-c>", nil)
-- uji.keymap.list()
