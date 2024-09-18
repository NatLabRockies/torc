##################
Restart a workflow
##################
Common cases that require you to restart a workflow include:

- Jobs failed because of a code or data bug.
- Jobs failed because they consumed more memory than expected.
- Jobs failed because they took longer than expected and the node allocation timed out.

The steps to restart a workflow are slightly different depending on whether the job actually
completed from torc's perspective and whether you defined job dependencies.

If a job completes with a return code, torc stores a result in the database. However, if the job
consumed all of the compute node's memory, the out-of-memory handler will terminate the process and
possibly the torc worker application. In these cases torc will not record a result. The job status
will still be ``submitted``.

Restarting jobs that did not finish
===================================
Run this command to re-initialize the workflow and relevant job statuses.

.. code-block:: console

   $ torc workflows restart

Check the status if you'd like.

.. code-block:: console

   $ torc jobs list -f status=ready

Schedule compute nodes to run those jobs.

.. code-block:: console

   $ torc hpc slurm schedule-nodes -nX

Where ``X`` is the number of Slurm jobs to schedule.

With job dependencies
=====================
If your jobs failed because of code and/or data bugs and

1. You fixed those bugs.
2. You defined dependencies on jobs for those files or data.

then you can the same steps above to re-initialize the job statuses and schedule new nodes.

Without job dependencies
========================
If you did not define job dependencies on files or data then you'll need to perform additional
steps to reset the status of jobs that you need to rerun.

If you want to rerun all jobs with a non-zero return code, run this command:

.. code-block:: console

   $ torc workflows reset-job-status --failed-only <workflow_key>

If you only want to rerun a subset of failed jobs, you will need to pass those job keys to this
command:

.. code-block:: console

   $ torc jobs reset-status KEY1 KEY2 ...

If the list of jobs to rerun is long then you'll want to employ some shell scripting. This command
will filter results by ``return_code=1``, return the output in JSON format, use the ``jq`` tool to
extract only job keys, and then pass those keys to the ``jobs reset-status`` command.

.. code-block:: console

   $ torc jobs reset-status $(torc -F json results list -f return_code=1 | jq -r '.results | .[] | .job_key')

Next, just as described above, run ``torc workflows restart`` and ``torc hpc slurm schedule-nodes``
to rerun the jobs.

Restarting jobs with --only-uninitialized
=========================================
By default the ``torc workflows restart`` command will reset the statuses of jobs are not ``done``.
This is because of cases where jobs and/or compute nodes timeout or fail. This may not be what you
want if you manually reset specific job statuses with ``torc jobs reset-status``.

You can run ``torc workflows restart --only-uninitialized`` instead. Only jobs with a status of
``uninitialized`` will get set to ``ready``.
