# Replacing everything

Copy the whole `lua/uji` folder of the uji source into
`~/.config/uji/lua/uji`. Every module then comes from your copy. A file you
delete from your copy falls back to the built-in one.

To run a complete tree kept somewhere else, set `UJI_RUNTIME` to the directory
that contains its `uji` folder. uji then uses none of its built-in files, and
replacements in your config and packs still apply on top.
