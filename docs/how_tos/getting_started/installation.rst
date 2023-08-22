.. _installation:

############
Installation
############

1. If running on NREL's HPC, contact Daniel Thom to procure a database. We are currently
   beta-testing and so only a limited number are available. You will be able to reach the database
   from any login or compute node. You can also install a database on a compute node, but obviously,
   it will only survive one compute node allocation. That can be sufficient for testing purposes.

2. If running on a local computer or cloud environment, install a database with ArangoDB. Refer to
   the links below.

3. Create a Python 3.10+ virtual environment (e.g., conda). If you are not familiar with Python
   virtual environments, install ``Miniconda`` (not ``Anaconda`` because it has unnecessary
   packages) by following instructions at
   https://conda.io/projects/conda/en/stable/user-guide/install/

.. code-block:: console

   $ conda create -y -n torc python=3.10
   $ conda activate torc

4. Install the Python package ``torc`` and its dependency ``resource_monitor``.

.. code-block:: console

    $ pip install git+ssh://git@github.nrel.gov/viz/wms.git@v0.1.14#subdirectory=torc_package \
        git+https://github.nrel.gov/dthom/resource_monitor@v0.1.2

Note that you can also install the ``torc`` package from a clone of the repository. This will give
you the latest code from the ``main`` branch.

.. code-block:: console

   $ git clone https://github.nrel.gov/viz/wms.git
   $ pip install -e wms/torc

5. Optionally install ``jq`` from https://stedolan.github.io/jq/download/ for parsing JSON.
   This tool is very useful when sending API requests with ``curl`` or dumping torc output to
   JSON.

Refer to :ref:`building-torc` for developer instructions on how to build the torc packages.
