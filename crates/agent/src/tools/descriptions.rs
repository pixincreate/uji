pub(crate) mod read_file {
    pub(crate) const TOOL: &str = "\
        Read a text file and return its contents with 1-based line numbers prefixed as `NNN| `. \
        Read a file before editing it so `edit_file` snippets match exactly. Long files come back \
        in pages; when there is more, the output ends with the offset to continue from. The line \
        numbers are display only - never include them in `edit_file` arguments.";
    pub(crate) const PATH: &str = "\
        Path to the file, absolute or relative to the working directory.";
    pub(crate) const OFFSET: &str = "1-based line to start at. Defaults to 1.";
    pub(crate) const LIMIT: &str = "Maximum number of lines to return.";
}

pub(crate) mod edit_file {
    pub(crate) const TOOL: &str = "\
        Replace an exact snippet of an existing file, leaving the rest untouched. This is the tool \
        to use for changing code. `old_string` must reproduce the file's current text byte for \
        byte, including indentation and newlines, and must appear exactly once unless \
        `replace_all` is true - include a few surrounding lines to make it unique. Do not include \
        the `NNN| ` line-number prefixes that read_file adds. To delete code, pass an empty \
        `new_string`.";
    pub(crate) const PATH: &str = "\
        Path to the file to edit, absolute or relative to the working directory.";
    pub(crate) const OLD_STRING: &str = "Exact text to find, copied verbatim from the file.";
    pub(crate) const NEW_STRING: &str = "\
        Text to put in its place. Empty string deletes the snippet.";
    pub(crate) const REPLACE_ALL: &str = "\
        Replace every occurrence instead of requiring exactly one. Defaults to false.";
}

pub(crate) mod write_file {
    pub(crate) const TOOL: &str = "\
        Write a file from scratch, creating parent directories as needed. This replaces the entire \
        file, so use it for new files only. To change an existing file use `edit_file` instead - \
        overwriting loses everything you did not include.";
    pub(crate) const PATH: &str = "Path to write, absolute or relative to the working directory.";
    pub(crate) const CONTENT: &str = "Complete contents of the file.";
}

pub(crate) mod run_command {
    pub(crate) const TOOL: &str = "\
        Run a shell command and return its combined stdout and stderr, plus the exit code when it \
        is non-zero. Every command starts in the working directory, so there is no need to `cd` \
        into it first. Use it to build, test, run linters, search with `rg`, `grep` or `find`, and \
        explore with `ls`. Read and change files with `read_file`, `edit_file` and `write_file`. \
        The command is non-interactive: it cannot prompt, and it is killed at the timeout.";
    pub(crate) const COMMAND: &str = "Shell command, for example `cargo test -p uji`.";
    pub(crate) const TIMEOUT: &str = "Seconds before the command is killed. Defaults to 120.";
}
