#!/bin/bash
# Fake srun for testing.
# Strips all srun option arguments and executes the remaining command directly.
# This simulates srun's behavior of running a command inside a Slurm step
# without requiring an actual Slurm installation.
#
# Handles:
#   --flag=value  (single arg, starts with -)
#   --flag        (boolean flag, starts with -)
#   -n 1          (short option with separate value)

while [[ "$1" == -* ]]; do
    # If this is a --key=value form, just shift once
    if [[ "$1" == *=* ]]; then
        shift
    # Short options that take a value as the next argument (e.g., -n 1, -N 2)
    elif [[ "$1" =~ ^-[a-zA-Z]$ ]]; then
        shift 2
    else
        # Boolean long flag like --overlap
        shift
    fi
done

exec "$@"
