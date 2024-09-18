.. _installation:

############
Installation
############

1. If running on NREL's HPC, contact Daniel Thom to procure a database on our shared server.
   If you would like to manage your own server, you can request a virtual machine from the HPC
   Operations team. We can help you setup and manage the database.

.. note:: The test database does not enable authentication for the torc service API. The database
   itself enables authentication but all torc CLI and API commands are not authenticated. This can
   be customized for other databases.

2. If running on a local computer or cloud environment, install a database with ArangoDB. Refer to
   the links below.

3. Create a Python 3.11+ virtual environment. This example uses the ``venv`` module in the standard
   library to create a virtual environment in your home directory.

   You may prefer ``conda`` or ``mamba``.

    .. code-block:: console

       $ module load python
       $ python -m venv ~/python-envs/torc

4. Activate the virtual environment.

    .. code-block:: console

       $ source ~/python-envs/torc/bin/activate

   Whenever you are done using torc, you can deactivate the environment by running ``deactivate``.

5. Install the Python package ``torc``.

    .. code-block:: console

        $ pip install git+ssh://git@github.nrel.gov/viz/torc.git@v0.4.2#subdirectory=torc_package

5. Optionally install the Julia client package.

    .. code-block:: console

        $ julia  # optionally specify an environment with --project
        $ using Pkg
        $ Pkg.add(PackageSpec(url="git@github.nrel.gov:viz/torc.git", rev="v0.4.2", subdir="julia/Torc"))

Note that you can also install the ``torc`` package from a clone of the repository. This will give
you the latest code from the ``main`` branch.

    .. code-block:: console

       $ git clone https://github.nrel.gov/viz/torc.git
       $ pip install -e torc/torc_package

6. Optionally install ``jq`` from https://stedolan.github.io/jq/download/ for parsing JSON.
   This tool is very useful when sending API requests with ``curl`` or dumping torc output to
   JSON.

Refer to :ref:`building-torc` for developer instructions on how to build the torc packages.
