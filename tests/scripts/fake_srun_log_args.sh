#!/bin/bash
# Fake srun that logs ALL arguments to a file, then executes the command.
# The log file path is passed via TORC_SRUN_ARGS_LOG.
# Each invocation appends one line: all arguments separated by spaces.

if [ -n "$TORC_SRUN_ARGS_LOG" ]; then
    echo "$@" >> "$TORC_SRUN_ARGS_LOG"
fi

# Now strip srun options and execute the command, same as fake_srun.sh
while [[ "$1" == -* ]]; do
    if [[ "$1" == *=* ]]; then
        shift
    elif [[ "$1" =~ ^-[a-zA-Z].+ ]]; then
        shift
    elif [[ "$1" =~ ^-[a-zA-Z]$ ]]; then
        shift 2
    else
        shift
    fi
done

exec "$@"
