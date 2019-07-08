# Redox's Parallel

Deploy the power of the Ion Shell in a multi-threaded fashion.

## How to
Run the command with `{}` where you want your arguments to be. Currently, only one argument is supported per run.

Example (Ion syntax):
```ion
parallel -progress 'echo {}' {1..1000}
```
or in POSIX shells (with the seq command):
```bash
parallel -progress 'echo {}' $(seq 1 999)
```
