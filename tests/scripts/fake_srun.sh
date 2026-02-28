#!/bin/bash
# Fake srun for testing.
# Strips all --flag arguments and executes the remaining command directly.
# This simulates srun's behavior of running a command inside a Slurm step
# without requiring an actual Slurm installation.

while [[ "$1" == --* ]]; do
    shift
done

exec "$@"
