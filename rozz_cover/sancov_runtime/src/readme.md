# Usage

## compile stage
``cargo build`` and get a static library ``sancovruntime.a``. link it when compile target programs using the following compilation flag:

```bash
export CFLAGS="-O2 -fsanitize-coverage=trace-pc-guard -Wl,--whole-archive sancovruntime.a -Wl,--no-whole-archive -pthread -lrt"
```

## runtime stage
First, start the ``sancov_server`` with

```bash
./sancov_server -p 1233 -o $SERVER_OUTPUT_FILE
``` 

and then, spawn a new terminal and set env var:

```bash
export REGISTRATION_ADDR="127.0.0.1:1233"
export SHM_DIR=$DIR_YOU_WANT_SHM_OUTPUT
export SESSION_ID=$ANY_INTEGER
```

and run your target program. After the program is terminated (normally or abnormally), the ``sancov_server`` will collect the coverage and serialize to ``$SERVER_OUTPUT_FILE``.