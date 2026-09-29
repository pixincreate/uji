# Tasks

Tasks let Lua functions wait without holding up uji. When one waits on the
network, a process, the keychain or a timer, the others and the screen carry
on. Your config, slash commands, key bindings, actions, tool functions and the
timers from `uji.schedule` and `uji.defer` are all tasks, so any of them can
wait.

Only one task runs at a time. It keeps running until it waits in a runtime
function that finishes later, in `uji.sleep`, in `promise:await()`, or in
`uji.task.race` and `uji.task.timeout`, and then another task gets its turn.
So a loop that never waits holds up everything. Waiting outside a task raises
an error, and so does waiting inside a coroutine you created yourself.

## uji.task.spawn(fn, ...)

Starts `fn` as a new task with the arguments that follow, and returns a task
object. It begins once the current task waits. Errors show as notices.

## task:cancel()

Stops the task where it waits, together with the functions that
`uji.task.race` and `uji.task.timeout` run for it. What it was waiting on stops
too, and uji closes the connections and processes that only this task used. A
task that cancels itself stops at its next wait.

## uji.sleep(seconds)

Pauses the task for `seconds`. Fractions work. With `0` the task pauses only
long enough for every other task that is ready to run first, which is how a
long loop can give the screen and the rest of uji their turn. Anything other
than a number of seconds raises an error.

## uji.task.race(fn, ...)

Runs every function at once as a task that belongs to the current task, and
waits for the first to finish. The result is its position followed by what it
returned, and the others stop there. An error in the first to finish is raised
again, and cancelling the current task stops all of them.

## uji.task.timeout(seconds, fn)

Runs `fn` as a task that belongs to the current task, for at most `seconds`.
When `fn` finishes in time the result is `true` followed by what it returned.
Otherwise `fn` stops and the result is `false`.

## uji.promise()

Creates a promise. Tasks wait on it until another task settles it.

| Member | Meaning |
|---|---|
| `promise:resolve(...)` | Settles the promise with the values given, and wakes every task waiting on it. The result is `true` the first time and `false` after. |
| `promise:await()` | Waits until the promise is settled and gives back its values. A settled promise gives them back at once. |
| `promise.settled` | `true` once the promise is settled. |
