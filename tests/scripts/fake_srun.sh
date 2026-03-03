#!/bin/bash
# Fake srun for testing.
# Strips all srun option arguments and executes the remaining command directly.
# This simulates srun's behavior of running a command inside a Slurm step
# without requiring an actual Slurm installation.
#
# Handles:
#   --flag=value   (single arg, starts with --)
#   --flag value   (boolean long flag, just shift)
#   -N2            (short option with value attached)
#   -n 1           (short option with separate value)

while [[ "$1" == -* ]]; do
    # --key=value form: single shift
    if [[ "$1" == *=* ]]; then
        shift
    # Short option with value attached (e.g., -N2, -n1): single shift
    elif [[ "$1" =~ ^-[a-zA-Z].+ ]]; then
        shift
    # Short option with separate value (e.g., -n 1, -N 2): shift twice
    elif [[ "$1" =~ ^-[a-zA-Z]$ ]]; then
        shift 2
    else
        # Boolean long flag like --overlap
        shift
    fi
done

exec "$@"
