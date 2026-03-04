#!/bin/bash

# Simple mock for scontrol
if [[ "$*" == *"show config"* ]]; then
    echo "ClusterName=test_cluster"
    exit 0
fi

if [[ "$*" == *"show partition"* ]]; then
    # Return basic info for any partition
    # Extract partition name from arguments (usually the 3rd argument after 'show partition')
    partition_name=$3
    echo "PartitionName=$partition_name MinNodes=1 MaxNodes=UNLIMITED OverSubscribe=NO QoS=N/A"
    exit 0
fi

exit 1
