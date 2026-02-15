# Usage
The scripts in this directory demonstrate how to run a partitioned Sienna\Ops simulation through
torc.

## Sienna simulation script
Create a simulation script with build and execute functions as defined in the
  [Sienna docs](https://nrel-sienna.github.io/PowerSimulations.jl/latest/modeler_guide/parallel_simulations/#Run-a-Simulation-in-Parallel-on-an-HPC). Refer to the example script `run_rts_uc_ed.jl`.

## Torc configuration script
The script `configure_parallel_simulation.jl` builds a torc workflow and adds it to the database.
It configures parameters for NLR's Kestrel HPC.

Configuration parameters to customize:
- TORC_SERVICE_URL: Adjust this for your environment.
- Sienna simulation script: At the bottom of the file there is a call to `configure_parallel_simulation`.
  The first parameter must be your Sienna simulation script. Configure the parameters in this
  function call as desired.
- Slurm account: Customize the instances of `SlurmSchedulerModel`. The example uses the debug
  partition to run the build job and the short partition for the work jobs.
- Resource requirements: Configure the instances of `ResourceRequirementsModel` for your jobs.
  This will determine how many jobs torc runs in parallel on each node. Memory is usually the limiting
  factor. For example, if each partitioned job needs 50 GB of memory, torc will will run 4 jobs in
  parallel on each Kestrel standard node (which has ~250 GB - overhead).
- Customize the `ComputeNodeScheduleParams.num_jobs` parameter. This tells torc how many compute nodes
  to schedule for the work jobs. Torc will schedule them after the build job completes.
- julia_env.sh: Edit this file if you need to perform different steps to make the julia binary available
  in the system path. You may not need the call to `module load julia` if you have configured julia with
  juliaup in your own environment.

## Build the workflow
```console
$ julia --project <your-env-path>
julia> include("configure_parallel_simulation.jl")
julia> configure_parallel_simulation(
    "run_RTS_UC-ED.jl",
    365,
    7,
    "simulation_output";
    num_overlap_steps=1,
    project_path=".",
    simulation_name="rts",
)
```
The torc workflow key will be printed to the console. Something like
```
Created Torc workflow key = 61160926
```

## Start the workflow and schedule nodes
```
$ torc -k 61160926 workflows start
```

This command will prompt you to choose a Slurm config. Select the config that will run
the build job. It will schedule the work jobs when complete, as discussed above.
```
$ torc -k 61160926 hpc slurm schedule-nodes -n1
```
