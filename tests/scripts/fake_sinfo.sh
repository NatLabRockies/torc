#!/bin/bash

# Simple mock for sinfo
if [[ "$*" == *"--version"* ]]; then
    echo "slurm 23.02.0"
    exit 0
fi

if [[ "$*" == *"-o %P|%c|%m|%l|%G|%D --noheader"* ]]; then
    echo "standard|104|246064|2-00:00:00|(null)|2112"
    echo "gpu|128|360000|2-00:00:00|gpu:h100:4|156"
    exit 0
fi

exit 1
