# First run

Start uji in the directory you want to work on.

```sh
cd ~/code/project
uji
```

## Signing in

Type `/login` and pick a provider from the list.

- Anthropic and OpenAI ask how you want to sign in. "Subscription (sign in
  with browser)" opens the provider's sign-in page, and "API key" asks for a
  key.
- A provider that needs a key asks for it. The prompt names the environment
  variable uji also reads, such as `ANTHROPIC_API_KEY`, and pressing Enter on
  an empty prompt uses that variable instead.
- Custom asks for the `base_url` and model of a server that takes OpenAI chat
  requests.

uji saves keys in the system keychain. On a system without one, they go in
`auth.json` in the [data directory](../configuration/files.md), readable only
by you.

## Choosing a model

`/models` lists the models of every provider you are signed in to. `/effort`
sets how much the model reasons, from off to high.

## Working with the model

- Enter sends your message. Shift+Enter, Alt+Enter or Ctrl+J start a new line.
- A message you send while the model works waits, and goes out when the turn
  ends.
- Esc clears the input line. On an empty line, Esc stops the turn.
- A line that starts with `!` runs in your shell, in the session's directory.
  Its output shows in the transcript, and the model does not see it.
- When a tool call needs your approval, `y` lets it run. `n` or Esc refuses
  it, and you can then tell the model what to do instead.

uji saves every conversation as a session.
[Commands](commands.md#running-uji) shows how to go back to one.
