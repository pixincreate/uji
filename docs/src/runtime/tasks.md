# Tasks

Tasks let Lua functions wait without holding up uji. When one waits on the
network, a process, the keychain or a timer, the others and the screen carry
on. Your config, slash commands, key bindings, actions, tool functions and the
timers from `uji.schedule` and `uji.defer` are all tasks, so any of them can
wait. Waiting from anywhere else raises an error.

## uji.task.spawn(fn, ...)

Starts `fn` as a new task with the arguments that follow, and returns a task
object. An error inside the task shows as a notice.

## task:cancel()

Stops the task. Connections and processes that only the task was using are
closed.

## uji.sleep(seconds)

Pauses the task for `seconds`. Fractions work.

## uji.task.race(fn, ...)

Runs every function at once inside the current task and waits for the first to
finish. The result is its position followed by what it returned, and the others
stop there. An error in the first to finish is raised again, and cancelling the
current task stops all of them.

## uji.task.timeout(seconds, fn)

Runs `fn` inside the current task for at most `seconds`. When `fn` finishes in
time the result is `true` followed by what it returned. Otherwise `fn` stops and
the result is `false`.

## uji.promise()

Creates a promise that tasks can wait on until another task settles it.

| Member | Meaning |
|---|---|
| `promise:resolve(...)` | Settles the promise with the values given. The result is `true` the first time and `false` after. |
| `promise:await()` | Waits until the promise is settled and gives back its values. |
| `promise.settled` | `true` once the promise is settled. |
