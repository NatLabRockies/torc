'use strict';
const {query} = require('@arangodb');
const db = require('@arangodb').db;
const {GiB, JobStatus, GRAPH_NAME} = require('./defs');
const schemas = require('./api/schemas');
const utils = require('./utils');
const config = require('./config');

/**
 * Add 'blocks' edges between jobs by looking at their file edges.
 * @param {Object} workflow
 */
function addBlocksEdgesFromFiles(workflow) {
  const graphName = config.getWorkflowGraphName(workflow);
  const blocksCollection = config.getWorkflowCollection(workflow, 'blocks');
  const needs = config.getWorkflowCollectionName(workflow, 'needs');
  const needsCollection = config.getWorkflowCollection(workflow, 'needs');
  const produces = config.getWorkflowCollectionName(workflow, 'produces');
  const files = config.getWorkflowCollection(workflow, 'files');
  for (const file of files.all()) {
    const fid = file._id;
    const fromVertices = query`
      FOR v
          IN 1
          INBOUND ${fid}
          GRAPH ${graphName}
          OPTIONS { edgeCollections: ${produces} }
          RETURN v._id
    `;
    if (fromVertices.count() > 0) {
      const toVertices = query`
        FOR v
            IN 1
            INBOUND ${file._id}
            GRAPH ${graphName}
            OPTIONS { edgeCollections: ${needs} }
            RETURN v._id
      `;
      for (const item of utils.product(
          fromVertices.toArray(),
          toVertices.toArray(),
      )) {
        const fromVertex = item[0];
        const toVertex = item[1];
        const cursor = query({count: true})`
            FOR edge IN ${needsCollection}
              FILTER edge._from == ${fromVertex} AND edge._to == ${toVertex}
              RETURN edge
            `;
        if (cursor.count() == 0) {
          blocksCollection.save({_from: fromVertex, _to: toVertex});
        }
      }
    }
  }
}

/**
 * Add 'blocks' edges between jobs by looking at their user_data consumes/stores edges.
 * @param {Object} workflow
 */
function addBlocksEdgesFromUserData(workflow) {
  const graphName = config.getWorkflowGraphName(workflow);
  const blocksCollection = config.getWorkflowCollection(workflow, 'blocks');
  const consumes = config.getWorkflowCollectionName(workflow, 'consumes');
  const consumesCollection = config.getWorkflowCollection(workflow, 'consumes');
  const stores = config.getWorkflowCollectionName(workflow, 'stores');
  const userData = config.getWorkflowCollection(workflow, 'user_data');
  for (const item of userData.all()) {
    const fromVertices = query`
      FOR v
          IN 1
          INBOUND ${item._id}
          GRAPH ${graphName}
          OPTIONS { edgeCollections: ${stores} }
          RETURN v._id
    `;
    if (fromVertices.count() > 0) {
      const toVertices = query`
        FOR v
            IN 1
            INBOUND ${item._id}
            GRAPH ${graphName}
            OPTIONS { edgeCollections: ${consumes} }
            RETURN v._id
      `;
      for (const item of utils.product(
          fromVertices.toArray(),
          toVertices.toArray(),
      )) {
        const fromVertex = item[0];
        const toVertex = item[1];
        const cursor = query({count: true})`
            FOR edge IN ${consumesCollection}
              FILTER edge._from == ${fromVertex} AND edge._to == ${toVertex}
              RETURN edge
            `;
        if (cursor.count() == 0) {
          blocksCollection.save({_from: fromVertex, _to: toVertex});
        }
      }
    }
  }
}

/**
 * Cancel all active jobs in the workflow.
 * @param {Object} workflow
 */
function cancelWorkflowJobs(workflow) {
  const collection = config.getWorkflowCollection(workflow, 'jobs');
  const collectionName = config.getWorkflowCollectionName(workflow, 'jobs');

  db._executeTransaction({
    collections: {
      exclusive: collectionName,
      allowImplicit: false,
    },
    action: function() {
      const cursor = query`
        FOR job IN ${collection}
          FILTER job.status == ${JobStatus.Submitted}
          || job.status == ${JobStatus.SubmittedPending}
          RETURN job
      `;

      for (const job of cursor) {
        job.status = JobStatus.Canceled;
        collection.update(job, job, {mergeObjects: false});
      }
    },
  });
}

/**
 * Get information about the resources required for currently-available jobs.
 * @param {Object} workflow
 * @param {string} schedulerConfigId
 * @return {Object}
 */
function getReadyJobRequirements(workflow, schedulerConfigId) {
  let numCpus = 0;
  let numGpus = 0;
  let numJobs = 0;
  let maxMemory = 0;
  let totalMemory = 0;
  let maxRuntime = 0;
  let maxRuntimeDuration = '';
  let maxNumNodes = 0;

  const collection = config.getWorkflowCollection(workflow, 'jobs');
  for (const job of collection.byExample({status: JobStatus.Ready})) {
    if (schedulerConfigId != null && job.internal.scheduler_config_id != schedulerConfigId) {
      continue;
    }
    const reqs = getJobResourceRequirements(job, workflow);
    numJobs += 1;
    numCpus += reqs.num_cpus;
    numGpus += reqs.num_gpus;
    const memory = utils.getMemoryInBytes(reqs.memory);
    totalMemory += memory;
    if (memory > maxMemory) {
      maxMemory = memory;
    }
    const runtime = utils.getTimeDurationInSeconds(reqs.runtime);
    if (runtime > maxRuntime) {
      maxRuntime = runtime;
      maxRuntimeDuration = reqs.runtime;
    }
    if (reqs.num_nodes > maxNumNodes) {
      maxNumNodes = reqs.num_nodes;
    }
  }
  return {
    num_jobs: numJobs,
    num_cpus: numCpus,
    num_gpus: numGpus,
    memory_gb: totalMemory / GiB,
    max_memory_gb: maxMemory / GiB,
    max_num_nodes: maxNumNodes,
    max_runtime: maxRuntimeDuration,
  };
}

/**
 * Return all jobs blocking this job.
 * @param {Object} job
 * @param {Object} workflow
 * @return {Array}
 */
function getBlockingJobs(job, workflow) {
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'blocks');
  const blockingJobs = [];
  const cursor = query`
    FOR v, e, p
      IN 1
      INBOUND ${job._id}
      GRAPH ${graphName}
      OPTIONS { edgeCollections: ${edgeName} }
      RETURN p.vertices[1]
  `;
  for (const job of cursor) {
    blockingJobs.push(job);
  }

  return blockingJobs;
}

/**
 * Return files needed by the passed job.
 * @param {Object} job
 * @param {Object} workflow
 * @return {ArangoQueryCursor}
 */
function listFilesNeededByJob(job, workflow) {
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'needs');
  return query({count: true})`
      FOR v
          IN 1
          OUTBOUND ${job._id}
          GRAPH ${graphName}
          OPTIONS { edgeCollections: ${edgeName} }
          RETURN v
    `;
}

/**
 * Return files needed by the passed job.
 * @param {Object} job
 * @param {Object} workflow
 * @return {ArangoQueryCursor}
 */
function listFilesProducedByJob(job, workflow) {
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'produces');
  return query({count: true})`
      FOR v
          IN 1
          OUTBOUND ${job._id}
          GRAPH ${graphName}
          OPTIONS { edgeCollections: ${edgeName} }
          RETURN v
    `;
}

/**
 * Return a job specification.
 * @param {Object} job - Instance of schemas.job
 * @param {Object} workflow
 * @return {Object} - Instance of schemas.jobSpecification
 */
function getJobSpecification(job, workflow) {
  const blockingJobs = [];
  const inputFiles = [];
  const outputFiles = [];
  const consumesUserData = [];
  const storesUserData = [];

  for (const blockingJob of getBlockingJobs(job, workflow)) {
    blockingJobs.push(blockingJob.name);
  }
  for (const file of listFilesNeededByJob(job, workflow)) {
    inputFiles.push(file.name);
  }
  for (const file of listFilesProducedByJob(job, workflow)) {
    outputFiles.push(file.name);
  }
  for (const data of listUserDataConsumedByJob(job, workflow)) {
    delete(data._id);
    delete(data._key);
    delete(data._rev);
    consumesUserData.push(data.name);
  }
  for (const data of listUserDataStoredByJob(job, workflow)) {
    delete(data._id);
    delete(data._key);
    delete(data._rev);
    storesUserData.push(data.name);
  }

  const scheduler = getJobScheduler(job, workflow);
  return {
    name: job.name,
    command: job.command,
    cancel_on_blocking_job_failure: job.cancel_on_blocking_job_failure,
    blocked_by: blockingJobs,
    input_files: inputFiles,
    output_files: outputFiles,
    needs_compute_node_schedule: job.needs_compute_node_schedule,
    resource_requirements: getJobResourceRequirements(job, workflow).name,
    scheduler: scheduler == null ? '' : scheduler._id,
    consumes_user_data: consumesUserData,
    stores_user_data: storesUserData,
  };
}

/**
 * Return the job's resource requirements, using default values if none are assigned.
 * @param {Object} job
 * @param {Object} workflow
 * @return {Object}
 */
function getJobResourceRequirements(job, workflow) {
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'requires');
  const cursor = query({count: true})`
    FOR v, e, p
      IN 1
      OUTBOUND ${job._id}
      GRAPH ${graphName}
      OPTIONS { edgeCollections: ${edgeName} }
      RETURN p.vertices[1]
  `;
  if (cursor.count() == 0) {
    const res = schemas.resourceRequirements.validate({name: 'default'});
    if (res.error !== null) {
      throw new Error('BUG: Failed to create default resourceRequirements');
    }
    return res.value;
  } else if (cursor.count() > 1) {
    // TODO: check at post
    throw new Error(
        'BUG: Only one resource requirement can be assigned to a job.',
    );
  }
  return cursor.toArray()[0];
}

/**
 * Set the job's resource requirements, removing any current requires edge.
 * @param {Object} job
 * @param {Object} resourceRequirements
 * @param {Object} workflow
 * @return {Object}
 */
function setOrReplaceJobResourceRequirements(job, resourceRequirements, workflow) {
  const graph = config.getWorkflowGraph(workflow);
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'requires');
  const requiresCollection = config.getWorkflowCollection(workflow, 'requires');
  const cursor = query({count: true})`
    FOR v, e
      IN 1
      OUTBOUND ${job._id}
      GRAPH ${graphName}
      OPTIONS { edgeCollections: ${edgeName} }
      RETURN e._id
  `;
  const count = cursor.count();
  if (count == 1) {
    graph[edgeName].remove(cursor.next());
  } else if (count > 1) {
    throw new Error(`Bug: requires edge count for ${job._id} cannot be greater than 1: ${count}`);
  }
  const edge = {_from: job._id, _to: resourceRequirements._id};
  const meta = requiresCollection.save(edge);
  Object.assign(edge, meta);
  return edge;
}

/**
 * Return the job's scheduler, returning null if one isn't defined.
 * @param {Object} job
 * @param {Object} workflow
 * @return {string}
 */
function getJobScheduler(job, workflow) {
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'scheduled_bys');
  const cursor = query({count: true})`
    FOR v, e, p
      IN 1
      OUTBOUND ${job._id}
      GRAPH ${graphName}
      OPTIONS { edgeCollections: ${edgeName} }
      RETURN p.vertices[1]
  `;
  if (cursor.count() == 0) {
    return null;
  } else if (cursor.count() > 1) {
    throw new Error('BUG: Only one scheduler can be assigned to a job.');
  }
  return cursor.toArray()[0];
}

/**
 * Return jobs that need the file.
 * @param {Object} file
 * @param {Object} workflow
 * @param {skip} skip
 * @param {number} limit
 * @return {ArangoQueryCursor}
 */
function getJobsThatNeedFile(file, workflow, skip, limit) {
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'needs');
  return query({count: true})`
      FOR v
          IN 1
          INBOUND ${file._id}
          GRAPH ${graphName}
          OPTIONS { edgeCollections: ${edgeName} }
          LIMIT ${skip}, ${limit}
          RETURN v
    `;
}

/**
 * Return all result documents connected to the job, sorted by completion time.
 * Return null if the job does not have a result.
 * @param {Object} job
 * @param {Object} workflow
 * @return {Object}
 */
function listJobResults(job, workflow) {
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'returned');
  const cursor = query({count: true})`
    FOR v, e, p
      IN 1
      OUTBOUND ${job._id}
      GRAPH ${graphName}
      OPTIONS { edgeCollections: ${edgeName} }
      RETURN p.vertices[1]
  `;
  const count = cursor.count();
  if (count == 0) {
    return null;
  }

  const results = cursor.toArray();
  if (results.length > 1) {
    results.sort(compareTimestamp);
  }
  return results;
}

/**
 * Comparison function for sorting timestamp strings.
 * @param {Object} result1
 * @param {Object} result2
 * @return {number}
 */
function compareTimestamp(result1, result2) {
  const t1 = new Date(result1.completion_time).getTime();
  const t2 = new Date(result2.completion_time).getTime();
  return t1 - t2;
}

/**
 * Return the latest job result.
 * Return null if the job does not have a result.
 * @param {Object} job
 * @param {Object} workflow
 * @return {Object}
 */
function getLatestJobResult(job, workflow) {
  const results = listJobResults(job, workflow);
  if (results == null) {
    return results;
  }

  return results.slice(-1)[0];
}

/**
 * Return job result for the given runId.
 * Return null if the job does not have a result for that runId.
 * @param {Object} job
 * @param {Object} workflow
 * @param {number} runId
 * @return {Object}
 */
function getJobResultByRunId(job, workflow, runId) {
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'returned');
  const cursor = query({count: true})`
    FOR v, e, p
      IN 1
      OUTBOUND ${job._id}
      GRAPH ${graphName}
      OPTIONS { edgeCollections: ${edgeName} }
      FILTER p.vertices[1].run_id == ${runId}
      RETURN p.vertices[1]
  `;
  const count = cursor.count();
  if (count == 0) {
    return null;
  } else if (count > 1) {
    throw new Error(`Bug: cannot have more than one result with a run_id: ${JSON.stringify(job)}`);
  }
  return cursor.next();
}

/**
 * Return the user data consumed by the job.
 * @param {Object} job
 * @param {Object} workflow
 * @return {ArangoQueryCursor}
 */
function listUserDataConsumedByJob(job, workflow) {
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'consumes');
  return query`
    FOR v, e, p
      IN 1
      OUTBOUND ${job._id}
      GRAPH ${graphName}
      OPTIONS { edgeCollections: ${edgeName} }
      RETURN p.vertices[1]
  `;
}

/**
 * Return the user data that is connected to the job.
 * @param {Object} job
 * @param {Object} workflow
 * @return {ArangoQueryCursor}
 */
function listUserDataStoredByJob(job, workflow) {
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'stores');
  return query`
    FOR v, e, p
      IN 1
      OUTBOUND ${job._id}
      GRAPH ${graphName}
      OPTIONS { edgeCollections: ${edgeName} }
      RETURN p.vertices[1]
  `;
}

/**
 * Return the user data with content in its data field.
 * @param {Object} workflow
 * @return {ArangoQueryCursor}
 */
function listUserDataWithEphemeralData(workflow) {
  const collection = config.getWorkflowCollection(workflow, 'user_data');
  return query`
    FOR doc in ${collection}
      FILTER doc.is_ephemeral && doc.data != NULL && LENGTH(doc.data) > 0
      RETURN doc
  `;
}

/**
 * Return the workflow config.
 * @param {Object} workflow
 * @return {Object}
*/
function getWorkflowConfig(workflow) {
  const cursor = query({count: true})`
    FOR v, e, p
      IN 1
      OUTBOUND ${workflow._id}
      GRAPH ${GRAPH_NAME}
      OPTIONS { edgeCollections: 'has_workflow_config' }
      RETURN p.vertices[1]
  `;
  if (cursor.count() != 1) {
    throw new Error(`workflow ${workflow._id} must only have one config: ${cursor.count()}`);
  }
  return cursor.next();
}

/**
 * Return the workflow status.
 * @param {Object} workflow
 * @return {Object}
*/
function getWorkflowStatus(workflow) {
  const cursor = query({count: true})`
    FOR v, e, p
      IN 1
      OUTBOUND ${workflow._id}
      GRAPH ${GRAPH_NAME}
      OPTIONS { edgeCollections: 'has_workflow_status' }
      RETURN p.vertices[1]
  `;
  if (cursor.count() != 1) {
    throw new Error(`workflow ${workflow._id} must only have one status: ${cursor.count()}`);
  }
  return cursor.next();
}

/** Set initial job statuses to blocked or ready. The default behavior changes all existing
 * statuses except JobStatus.Disabled. Set onlyUninitialized=true if the user is managing
 * existing statuses on a workflow restart.
 * @param {Object} workflow
 * @param {Boolean} onlyUninitialized
 */
function initializeJobStatus(workflow, onlyUninitialized) {
  // TODO: Can this be more efficient with one traversal?
  const jobs = config.getWorkflowCollection(workflow, 'jobs');
  const schedulers = getSchedulers(workflow);
  for (const job of jobs.all()) {
    const jobResources = getJobResourceRequirements(job, workflow);
    if (job.internal == null) {
      job.internal = schemas.jobInternal.validate({}).value;
    }
    const scheduler = getJobScheduler(job, workflow);
    if (scheduler == null) {
      job.internal.scheduler_config_id = selectBestSchedulerForJob(job, schedulers)._id;
    } else {
      job.internal.scheduler_config_id = scheduler._id;
    }
    job.internal.memory_bytes = utils.getMemoryInBytes(jobResources.memory);
    job.internal.runtime_seconds = utils.getTimeDurationInSeconds(jobResources.runtime);
    job.internal.num_cpus = jobResources.num_cpus;
    job.internal.num_gpus = jobResources.num_gpus;
    job.internal.num_nodes = jobResources.num_nodes;
    if (
      job.status != JobStatus.Disabled &&
        (!onlyUninitialized || job.status == JobStatus.Uninitialized)
    ) {
      if (isJobInitiallyBlocked(job, workflow)) {
        job.status = JobStatus.Blocked;
      } else if (job.status != JobStatus.Done) {
        job.status = JobStatus.Ready;
      }
    }
    jobs.update(job, job, {mergeObjects: false});
  }
  console.log(
      `Initialized all incomplete job status to ${JobStatus.Ready} or ${JobStatus.Blocked}`,
  );
}

/**
 * Return the ID of the best scheduler for job. Barely functional.
 * @param {Object} job
 * @param {Array} schedulers
 * @return {string}
 */
function selectBestSchedulerForJob(job, schedulers) {
  const allSchedulers = schedulers.slurmSchedulers.concat(
      schedulers.awsSchedulers,
      schedulers.localSchedulers,
  );
  if (allSchedulers.length == 1) {
    const scheduler = allSchedulers[0];
    if (schedulers.slurmSchedulers.length == 1) {
      if (job.internal.runtime_seconds <= schedulers.durationToSeconds.get(scheduler.walltime)) {
        // TODO: this really needs to convert all Slurm strings into common units and then compare.
        return scheduler;
      }
    } else {
      return scheduler;
    }
  }
  // TODO: Figure out the best when there are multiple matches.
  return '';
}

/**
 * Return all stored schedulers.
 * @param {Object} workflow
 * @return {Object}
 */
function getSchedulers(workflow) {
  const schedulers = {
    awsSchedulers: config.getWorkflowCollection(workflow, 'aws_schedulers').toArray(),
    localSchedulers: config.getWorkflowCollection(workflow, 'local_schedulers').toArray(),
    slurmSchedulers: config.getWorkflowCollection(workflow, 'slurm_schedulers').toArray(),
    durationToSeconds: new Map(),
  };
  for (const scheduler of schedulers.slurmSchedulers) {
    if (!schedulers.durationToSeconds.has(scheduler.walltime)) {
      const durationSeconds = utils.getWalltimeInSeconds(scheduler.walltime);
      schedulers.durationToSeconds.set(scheduler.walltime, durationSeconds);
    }
  }
  return schedulers;
}

/**
 * Return an iterator over all documents of the given type connected to the workflow.
 * @param {Object} workflow
 * @param {string} collectionName
 * @return {Object}
 */
function iterWorkflowDocuments(workflow, collectionName) {
  const collection = config.getWorkflowCollection(workflow, collectionName);
  return collection.all();
}

/**
 * Return true if the job is blocked by another job.
 * @param {Object} job
 * @param {Object} workflow
 * @return {bool}
 *
 **/
function isJobBlocked(job, workflow) {
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'blocks');
  const cursor = query`
    FOR v
        IN 1
        INBOUND ${job._id}
        GRAPH ${graphName}
        OPTIONS { edgeCollections: ${edgeName} }
        RETURN v.status
  `;
  for (const status of cursor) {
    if (!isJobStatusComplete(status)) {
      return true;
    }
  }

  return false;
}

/**
 * Return true if the job is initially blocked by another job.
 * @param {Object} job
 * @param {Object} workflow
 * @return {bool}
 *
 **/
function isJobInitiallyBlocked(job, workflow) {
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'blocks');
  const cursor = query({count: true})`
    FOR v
        IN 1
        INBOUND ${job._id}
        GRAPH ${graphName}
        OPTIONS { edgeCollections: ${edgeName} }
        FILTER v.status != ${JobStatus.Done}
        RETURN v._id
  `;
  return cursor.count() > 0;
}

/**
 * Return true if the job status indicates completion.
 * @param {string} status
 * @return {bool}
 */
function isJobStatusComplete(status) {
  return status == JobStatus.Done || status == JobStatus.Canceled ||
    status == JobStatus.Terminated;
}

/**
 * Return true if the workflow is complete.
 * @param {Object} workflow
 * @return {bool}
 */
function isWorkflowComplete(workflow) {
  // TODO: This function will be called a lot - by every compute node on some interval.
  // May need to ensure that jobs or at least their status are always cached.
  // Or track job completions, which could easily end up being wrong.
  const collection = config.getWorkflowCollection(workflow, 'jobs');
  const cursor = query({count: true})`
    FOR job in ${collection}
      FILTER !(
        job.status == ${JobStatus.Done}
        OR job.status == ${JobStatus.Canceled}
        OR job.status == ${JobStatus.Terminated}
        OR job.status == ${JobStatus.Disabled}
      )
      LIMIT 1
      RETURN job._key
  `;
  return cursor.count() == 0;
}

/**
 * Return the job's process stats.
 * @param {Object} job
 * @param {Object} workflow
 * @return {Array} Array of jobProcessStats
 */
function listJobProcessStats(job, workflow) {
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'process_used');
  const cursor = query({count: true})`
    FOR v, e, p
      IN 1
      OUTBOUND ${job._id}
      GRAPH ${graphName}
      OPTIONS { edgeCollections: ${edgeName} }
      RETURN p.vertices[1]
  `;
  const results = cursor.toArray();
  if (results.length > 1) {
    results.sort((x, y) => x.run_id - y.run_id);
  }
  return results;
}

/**
 * Join two collections by an inbound edge.
 * @param {Object} workflow
 * @param {string} fromCollectionBase
 * @param {string} edgeBase
 * @param {Object} filters
 * @param {number} skip
 * @param {number} limit
 * @return {Object}
 */
function joinCollectionsByInboundEdge(
    workflow,
    fromCollectionBase,
    edgeBase,
    filters,
    skip,
    limit,
) {
  const graphName = config.getWorkflowGraphName(workflow);
  let fromCollection = config.getWorkflowCollection(workflow, fromCollectionBase);
  if (filters != null && Object.keys(filters).length > 0) {
    fromCollection = fromCollection.byExample(filters);
  } else {
    fromCollection = fromCollection.all();
  }
  fromCollection = fromCollection.skip(skip)
      .limit(limit)
      .toArray();
  const edgeName = config.getWorkflowCollectionName(workflow, edgeBase);
  // TODO: It would be better to allow dynamic filtering of fields on either
  // side of the edge in this query.
  const cursor = query({count: true})`
    FOR x in ${fromCollection}
      FOR v, e, p
          IN 1
          INBOUND x._id
          GRAPH ${graphName}
          OPTIONS { edgeCollections: ${edgeName}, uniqueVertices: 'global', order: 'bfs' }
          RETURN {from: p.vertices[1], to: x}
  `;
  return cursor;
}

/**
 * Join two collections by an outbound edge.
 * @param {Object} workflow
 * @param {string} fromCollectionBase
 * @param {string} edgeBase
 * @param {Object} filters
 * @param {number} skip
 * @param {number} limit
 * @return {Object}
 */
function joinCollectionsByOutboundEdge(
    workflow,
    fromCollectionBase,
    edgeBase,
    filters,
    skip,
    limit,
) {
  const graphName = config.getWorkflowGraphName(workflow);
  let fromCollection = config.getWorkflowCollection(workflow, fromCollectionBase);
  if (filters != null && Object.keys(filters).length > 0) {
    fromCollection = fromCollection.byExample(filters);
  } else {
    fromCollection = fromCollection.all();
  }
  fromCollection = fromCollection.skip(skip)
      .limit(limit)
      .toArray();
  const edgeName = config.getWorkflowCollectionName(workflow, edgeBase);

  const cursor = query({count: true})`
    FOR x in ${fromCollection}
      FOR v, e, p
          IN 1
          OUTBOUND x._id
          GRAPH ${graphName}
          OPTIONS { edgeCollections: ${edgeName}, uniqueVertices: 'global', order: 'bfs' }
          RETURN {from: x, to: p.vertices[1]}
  `;
  return cursor;
}

/**
 * Return an array of file keys and revisions needed by the job.
 * @param {Object} job
 * @param {Object} workflow
 * @return {Array}
 */
function listNeedsFileRevisions(job, workflow) {
  const items = [];
  for (const item of listFilesNeededByJob(job, workflow)) {
    items.push({key: item._key, rev: item._rev});
  }
  items.sort((x, y) => x.key - y.key);
  return items;
}

/**
 * Return an array of user data keys and revisions consumed by the job.
 * @param {Object} job
 * @param {Object} workflow
 * @return {Array}
 */
function listConsumesUserDataRevisions(job, workflow) {
  const items = [];
  for (const item of listUserDataConsumedByJob(job, workflow)) {
    items.push({key: item._key, rev: item._rev});
  }
  items.sort((x, y) => x.key - y.key);
  return items;
}

/**
 * Return an array of user data keys and revisions stored by the job.
 * @param {Object} job
 * @param {Object} workflow
 * @return {Array}
 */
function listStoresUserDataRevisions(job, workflow) {
  const items = [];
  for (const item of listUserDataStoredByJob(job, workflow)) {
    items.push({key: item._key, rev: item._rev});
  }
  items.sort((x, y) => x.key - y.key);
  return items;
}

/**
 * Return an array of user_data keys whose data should exist but doesn't.
 * @param {Object} workflow
 * @return {Array}
 */
function listMissingUserData(workflow) {
  const expectedUserData = [];
  const missingUserData = [];
  const consumesCollection = config.getWorkflowCollection(workflow, 'consumes');
  const jobsCollection = config.getWorkflowCollection(workflow, 'jobs');
  const storesCollection = config.getWorkflowCollection(workflow, 'stores');
  const userDataCollection = config.getWorkflowCollection(workflow, 'user_data');

  for (const edge of consumesCollection.all()) {
    const udId = edge._to;
    const storesEdge = storesCollection.byExample({_to: udId}).toArray();
    if (storesEdge.length == 0) {
      // This user data should have been added by the user.
      expectedUserData.push(udId);
    } else if (storesEdge.length == 1) {
      const jobId = storesEdge[0]._from;
      const job = jobsCollection.document(jobId);
      if (job.status == JobStatus.Done) {
        // This user data should have been added by the job.
        expectedUserData.push(udId);
      }
    } else {
      throw new Error(`A user_data document can never have more than 1 stores edge: ` +
        `${JSON.stringify(storesEdge)}`);
    }
  }

  for (const udId of expectedUserData) {
    const ud = userDataCollection.document(udId);
    if (Object.keys(ud.data).length == 0) {
      missingUserData.push(ud._key);
    }
  }
  return missingUserData;
}

/**
 * Return an array of file keys whose path must exist.
 * @param {Object} workflow
 * @return {Array}
 */
function listRequiredExistingFiles(workflow) {
  const requiredFiles = [];
  const needsCollection = config.getWorkflowCollection(workflow, 'needs');
  const jobsCollection = config.getWorkflowCollection(workflow, 'jobs');
  const producesCollection = config.getWorkflowCollection(workflow, 'produces');

  for (const edge of needsCollection.all()) {
    const fileId = edge._to;
    const producesEdge = producesCollection.byExample({_to: fileId}).toArray();
    if (producesEdge.length == 0) {
      // This file should have been created by the user.
      requiredFiles.push(fileId);
    } else if (producesEdge.length == 1) {
      const jobId = producesEdge[0]._from;
      const job = jobsCollection.document(jobId);
      if (job.status == JobStatus.Done) {
        // This user data should have been added by the job.
        requiredFiles.push(fileId);
      }
    } else {
      throw new Error(`A file document can never have more than 1 produces edge: ` +
        `${JSON.stringify(producesEdge)}`);
    }
  }

  return requiredFiles;
}

/**
 * Update a job status and manage all downstream consequences.
 * @param {Object} job
 * @param {Object} workflow
 * @return {Object}
 */
function manageJobStatusChange(job, workflow) {
  const jobs = config.getWorkflowCollection(workflow, 'jobs');
  const oldStatus = jobs.document(job._key).status;
  if (job.status == oldStatus) {
    return job;
  }
  const meta = jobs.update(job, job, {mergeObjects: false});
  Object.assign(job, meta);

  const workflowStatus = getWorkflowStatus(workflow);
  if (!isJobStatusComplete(oldStatus) && isJobStatusComplete(job.status)) {
    const result = getJobResultByRunId(job, workflow, workflowStatus.run_id);
    if (result == null) {
      throw new Error(
          `A job must have a result before it is completed: ${job._key}.`,
      );
    }
    updateBlockedJobsFromCompletion(job, workflow);
  } else if (isJobStatusComplete(oldStatus) && job.status == JobStatus.Uninitialized) {
    updateJobsFromCompletionReversal(job, workflow);
  }
  return job;
}

/**
 * Prepare a list of jobs for submission that meet the worker resource availability.
 * @param {Object} workflow
 * @param {Object} workerResources
 * @param {Number} limit
 * @param {Object} reason
 * @return {Array}
 */
function prepareJobsForSubmission(workflow, workerResources, limit, reason) {
  const jobs = [];
  const collection = config.getWorkflowCollection(workflow, 'jobs');
  const collectionName = config.getWorkflowCollectionName(workflow, 'jobs');
  let availableCpus = workerResources.num_cpus;
  let availableGpus = workerResources.num_gpus;
  let availableMemory = workerResources.memory_gb * GiB;
  const queryLimit = limit == null ? availableCpus : limit;
  const workerTimeLimit =
    workerResources.time_limit == null ?
      Number.MAX_SAFE_INTEGER : utils.getTimeDurationInSeconds(workerResources.time_limit);
  const schedulerConfigId = workerResources.scheduler_config_id == null ? '' :
    workerResources.scheduler_config_id;

  db._executeTransaction({
    collections: {
      exclusive: collectionName,
      allowImplicit: false,
    },
    action: function() {
      const cursor = query({count: true})`
        FOR job IN ${collection}
          FILTER (job.status == ${JobStatus.Ready} || job.status == ${JobStatus.Scheduled})
            && job.internal.memory_bytes <= ${availableMemory}
            && job.internal.num_cpus <= ${availableCpus}
            && job.internal.num_gpus <= ${availableGpus}
            && job.internal.runtime_seconds <= ${workerTimeLimit}
            && job.internal.num_nodes == ${workerResources.num_nodes}
            && (${schedulerConfigId} == '' || job.internal.scheduler_config_id == ''
            || job.internal.scheduler_config_id == ${schedulerConfigId})
          LIMIT ${queryLimit}
          RETURN job
      `;

      // This implementation stores the job resource information in the internal object
      // so that it doesn't have to run a graph query while holding an exclusive lock.
      for (const job of cursor) {
        if (
          job.internal.num_cpus <= availableCpus &&
          job.internal.num_gpus <= availableGpus &&
          job.internal.memory_bytes <= availableMemory
        ) {
          job.status = JobStatus.SubmittedPending;
          const meta = collection.update(job, job, {mergeObjects: false});
          Object.assign(job, meta);
          jobs.push(job);
          availableCpus -= job.internal.num_cpus;
          availableGpus -= job.internal.num_gpus;
          availableMemory -= job.internal.memory_bytes;
          if (availableCpus == 0 || availableMemory == 0) {
            break;
          }
        }
      }
    },
  });

  if (jobs.length == 0) {
    reason.message = `No jobs matched status='ready', memory_bytes <= ${availableMemory}, ` +
      `num_cpus <= ${availableCpus}, num_gpus <= ${availableGpus}, ` +
      `runtime_seconds <= ${workerTimeLimit}, ` +
      `num_nodes == ${workerResources.num_nodes}, scheduler_config_id == ${schedulerConfigId}`;
  }
  // console.log(`Prepared ${jobs.length} jobs for submission.`);
  return jobs;
}

/**
 * Prepare a list of jobs for submission with no resource requirement considerations.
 * @param {Object} workflow
 * @param {Number} limit
 * @return {Array}
 */
function prepareJobsForSubmissionNoResourceChecks(workflow, limit) {
  const jobs = [];
  const collection = config.getWorkflowCollection(workflow, 'jobs');
  const collectionName = config.getWorkflowCollectionName(workflow, 'jobs');

  db._executeTransaction({
    collections: {
      exclusive: collectionName,
      allowImplicit: false,
    },
    action: function() {
      const cursor = query({count: true})`
        FOR job IN ${collection}
          FILTER job.status == ${JobStatus.Ready} || job.status == ${JobStatus.Scheduled}
          LIMIT ${limit}
          RETURN job
      `;

      // This implementation stores the job resource information in the internal object
      // so that it doesn't have to run a graph query while holding an exclusive lock.
      for (const job of cursor) {
        job.status = JobStatus.SubmittedPending;
        const meta = collection.update(job, job, {mergeObjects: false});
        Object.assign(job, meta);
        jobs.push(job);
      }
    },
  });

  return jobs;
}

/**
 * Return an array of schedulers that need to be scheduled. Changes job status to scheduled.
 * @param {Object} workflow
 * @return {Array}
 */
function prepareJobsForScheduling(workflow) {
  const schedulers = [];
  const collection = config.getWorkflowCollection(workflow, 'jobs');
  const collectionName = config.getWorkflowCollectionName(workflow, 'jobs');

  db._executeTransaction({
    collections: {
      exclusive: collectionName,
      allowImplicit: false,
    },
    action: function() {
      const cursor = query({count: true})`
        FOR job IN ${collection}
          FILTER job.status == ${JobStatus.Ready} && job.needs_compute_node_schedule
          RETURN job
      `;

      for (const job of cursor) {
        job.status = JobStatus.Scheduled;
        const meta = collection.update(job, job, {mergeObjects: false});
        Object.assign(job, meta);
        schedulers.push(job.internal.scheduler_config_id);
      }
    },
  });

  return schedulers;
}

/**
 * Process user data that was consumed by jobs and later changed, and reinitialize job status.
 * There is no protection for concurrent requests.
 * @param {Object} workflow
 * @return {Array}
 */
function processConsumedUserData(workflow) {
  const consumesCollection = config.getWorkflowCollection(workflow, 'consumes');
  const jobsCollection = config.getWorkflowCollection(workflow, 'jobs');
  const userDataCollection = config.getWorkflowCollection(workflow, 'user_data');
  const reinitializedJobs = [];

  for (const edge of consumesCollection.all()) {
    const ud = userDataCollection.document(edge._to);
    if (ud._rev != edge.consumed_revision) {
      const job = jobsCollection.document(edge._from);
      if (job.status == JobStatus.Done) {
        job.status = JobStatus.Uninitialized;
        jobsCollection.update(job, job, {mergeObjects: false});
        reinitializedJobs.push(job._key);
      }
    }
  }
  return reinitializedJobs;
}

/** Reset job status to uninitialized.
 * @param {Object} workflow
 * @param {boolean} failedOnly
 */
function resetJobStatus(workflow, failedOnly) {
  const jobsCollection = config.getWorkflowCollection(workflow, 'jobs');
  const workflowStatus = getWorkflowStatus(workflow);

  // PERF: this could be one query.
  for (const job of jobsCollection.all()) {
    if (failedOnly) {
      const result = getJobResultByRunId(job, workflow, workflowStatus.run_id);
      // This only includes jobs that completed and failed.
      // Some jobs may not have run because they were already successful.
      // Jobs that did not complete will get reset in the workflow restart command.
      if (result == null || result.return_code == 0) {
        continue;
      }
    }
    job.status = JobStatus.Uninitialized;
    jobsCollection.update(job, job, {mergeObjects: false});
  }
  console.log(`Reset job status to ${JobStatus.Uninitialized}`);
}

/** Reset workflow config.
 * @param {Object} workflow
 */
function resetWorkflowConfig(workflow) {
  const config = {
    compute_node_resource_stats: schemas.computeNodeResourceStatConfig.validate({}).value,
  };

  const doc = getWorkflowConfig(workflow);
  if (doc == null) {
    db.workflow_config.save(config);
  } else {
    Object.assign(doc, config);
    db.workflow_config.update(doc, doc, {mergeObjects: false});
  }
}

/** Reset workflow status.
 * @param {Object} workflow
 */
function resetWorkflowStatus(workflow) {
  const status = {
    is_canceled: false,
    scheduled_compute_node_ids: [],
    auto_tune_status: schemas.autoTuneStatus.validate({}).value,
  };
  const doc = getWorkflowStatus(workflow);
  Object.assign(doc, status);
  db.workflow_statuses.update(doc, doc, {mergeObjects: false});
  console.log(`Reset workflow status`);
}

/**
 * Setup the jobs to auto-tune resource requirements.
 * Enable one job from each resource requirement group and disable the rest.
 * @param {Object} workflow
 */
function setupAutoTuneResourceRequirements(workflow) {
  const groups = new Set();
  const status = getWorkflowStatus(workflow);
  status.auto_tune_status.enabled = true;

  const workflowConfig = getWorkflowConfig(workflow);
  if (!workflowConfig.compute_node_resource_stats.process) {
    throw new Error('The auto-tune feature requires collection of job process stats.');
  }

  const jobs = config.getWorkflowCollection(workflow, 'jobs');
  for (const job of jobs.all()) {
    if (job.status == JobStatus.Blocked) {
      continue;
    }
    const rr = getJobResourceRequirements(job, workflow);
    if (groups.has(rr.name)) {
      if (job.status == JobStatus.Disabled) {
        // TODO: should we track these instead?
        res.throw(400, `Job ${job._key} is already disabled`);
      }
      // This isn't atomic, but the user shouldn't call this in parallel.
      // Let Arango fail the operation if they do that.
      job.status = JobStatus.Disabled;
      jobs.update(job, job, {mergeObjects: false});
    } else {
      status.auto_tune_status.job_keys.push(job._key);
      groups.add(rr.name);
    }
  }
  db.workflow_statuses.update(status, status, {mergeObjects: false});
}

/**
 * Process the results of setupAutoTuneResourceRequirements.
 * 1. Update the resource requirements groups based on the utilization stats from
 * the selected jobs that ran.
 * 2. Update all non-auto-tune job status from disabled to uninitialized.
 * @param {Object} workflow
 */
function processAutoTuneResourceRequirementsResults(workflow) {
  const workflowStatus = getWorkflowStatus(workflow);
  workflowStatus.auto_tune_status.disabled = true;
  const groupsUpdated = new Set();
  const autoTuneJobs = new Set();
  const jobs = config.getWorkflowCollection(workflow, 'jobs');

  // FUTURE: consider whether all changes can be made atomically.
  for (const key of workflowStatus.auto_tune_status.job_keys) {
    const job = jobs.document(key);
    const jobStats = listJobProcessStats(job, workflow);
    if (jobStats.length == 0) {
      throw new Error(`job ${job._key} does not have any process stats`);
    }
    const stats = jobStats.slice(-1)[0];
    const maxMemoryGb = Math.ceil(stats.max_rss / GiB);
    const maxMemory = `${maxMemoryGb}g`;
    const maxCpusUsed = stats.max_cpu_percent == 0 ? 1 : Math.ceil(stats.max_cpu_percent / 100);
    const rr = getJobResourceRequirements(job, workflow);
    const result = getJobResultByRunId(job, workflow, workflowStatus.run_id);
    if (result == null) {
      throw new Error(`No job result for ${job._key} - ${job.status}. Cannot complete auto-tune.`);
    }
    const oldRr = JSON.parse(JSON.stringify(rr));
    rr.num_cpus = maxCpusUsed;
    rr.memory = maxMemory;
    const minutes = Math.ceil(result.exec_time_minutes);
    rr.runtime = `P0DT0H${minutes}M`;
    if (groupsUpdated.has(rr.name)) {
      throw new Error(`resource requirements ${rr.name} was already updated`);
    }
    groupsUpdated.add(rr.name);
    const rrCollection = config.getWorkflowCollection(workflow, 'resource_requirements');
    rrCollection.update(rr, rr, {mergeObjects: false});
    const event = {
      timestamp: (new Date()).toISOString(),
      category: 'resource_requirements',
      type: 'update',
      name: rr.name,
      old: oldRr,
      new: rr,
      message: `Updated resource requirements for name = ${rr.name}`,
    };
    const eventsCollection = config.getWorkflowCollection(workflow, 'events');
    eventsCollection.save(event);
    autoTuneJobs.add(job._key);
  }

  const jobsCollection = config.getWorkflowCollection(workflow, 'jobs');
  for (const job of jobsCollection.all()) {
    if (!autoTuneJobs.has(job._key)) {
      if (job.status != JobStatus.Disabled) {
        throw new Error(`Expected status disabled instead of ${job.status} for job ${job._key}`);
      }
      job.status = JobStatus.Uninitialized;
      jobsCollection.update(job, job, {mergeObjects: false});
    }
  }
  db.workflow_statuses.update(workflowStatus, workflowStatus, {mergeObjects: false});
}

/**
 * Update blocked jobs after a job completion.
 * @param {Object} job
 * @param {Object} workflow
 */
function updateBlockedJobsFromCompletion(job, workflow) {
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'blocks');
  const cursor = query`
    FOR v, e, p
      IN 1
      OUTBOUND ${job._id}
      GRAPH ${graphName}
      OPTIONS { edgeCollections: ${edgeName}, uniqueVertices: 'global', order: 'bfs' }
      RETURN p.vertices[1]
  `;
  const workflowStatus = getWorkflowStatus(workflow);
  const result = getJobResultByRunId(job, workflow, workflowStatus.run_id);
  // TODO: should other queries use bfs?
  const jobs = config.getWorkflowCollection(workflow, 'jobs');
  for (const blockedJob of cursor) {
    if (!isJobBlocked(blockedJob, workflow)) {
      if (result.return_code != 0 && blockedJob.cancel_on_blocking_job_failure) {
        blockedJob.status = JobStatus.Canceled;
      } else {
        blockedJob.status = JobStatus.Ready;
      }
      jobs.update(blockedJob, blockedJob, {mergeObjects: false});
    }
  }
}

/**
 * Update jobs after a job completion reversal.
 * @param {Object} job
 * @param {Object} workflow
 */
function updateJobsFromCompletionReversal(job, workflow) {
  const graphName = config.getWorkflowGraphName(workflow);
  const edgeName = config.getWorkflowCollectionName(workflow, 'blocks');
  const jobs = config.getWorkflowCollection(workflow, 'jobs');
  const numJobs = jobs.count();
  const cursor = query`
    FOR v, e, p
      IN 1..${numJobs}
      OUTBOUND ${job._id}
      GRAPH ${graphName}
      OPTIONS { edgeCollections: ${edgeName}, uniqueVertices: 'global', order: 'bfs' }
      RETURN v
  `;
  for (const downstreamJob of cursor) {
    if (downstreamJob.status != JobStatus.Uninitialized) {
      downstreamJob.status = JobStatus.Uninitialized;
      jobs.update(downstreamJob, downstreamJob, {mergeObjects: false});
      console.log(`Reset job=${downstreamJob._key} status to ${JobStatus.Uninitialized}`);
    }
  }
}

/**
 * Update workflow config.
 * @param {Object} workflow
 * @param {Object} config
 * @return {Object}
 **/
function updateWorkflowConfig(workflow, config) {
  const doc = getWorkflowConfig(workflow);
  Object.assign(doc, config);
  Object.assign(doc.compute_node_resource_stats, config.compute_node_resource_stats);
  const meta = db.workflow_configs.update(doc, doc, {mergeObjects: false});
  Object.assign(doc, meta);
  return doc;
}

module.exports = {
  addBlocksEdgesFromFiles,
  addBlocksEdgesFromUserData,
  cancelWorkflowJobs,
  getBlockingJobs,
  getJobResourceRequirements,
  getJobResultByRunId,
  getJobScheduler,
  getJobsThatNeedFile,
  getLatestJobResult,
  getReadyJobRequirements,
  getWorkflowConfig,
  getWorkflowStatus,
  getJobSpecification,
  initializeJobStatus,
  isJobBlocked,
  isJobInitiallyBlocked,
  isJobStatusComplete,
  isWorkflowComplete,
  iterWorkflowDocuments,
  joinCollectionsByInboundEdge,
  joinCollectionsByOutboundEdge,
  listConsumesUserDataRevisions,
  listFilesNeededByJob,
  listFilesProducedByJob,
  listJobProcessStats,
  listJobResults,
  listMissingUserData,
  listNeedsFileRevisions,
  listRequiredExistingFiles,
  listStoresUserDataRevisions,
  listUserDataConsumedByJob,
  listUserDataStoredByJob,
  listUserDataWithEphemeralData,
  manageJobStatusChange,
  prepareJobsForSubmission,
  prepareJobsForSubmissionNoResourceChecks,
  prepareJobsForScheduling,
  processAutoTuneResourceRequirementsResults,
  processConsumedUserData,
  resetJobStatus,
  resetWorkflowConfig,
  resetWorkflowStatus,
  setOrReplaceJobResourceRequirements,
  setupAutoTuneResourceRequirements,
  updateBlockedJobsFromCompletion,
  updateWorkflowConfig,
};
