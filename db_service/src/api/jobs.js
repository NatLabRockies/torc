'use strict';
const joi = require('joi');
const {MAX_TRANSFER_RECORDS, JobStatus} = require('../defs');
const config = require('../config');
const documents = require('../documents');
const utils = require('../utils');
const query = require('../query');
const schemas = require('./schemas');
const createRouter = require('@arangodb/foxx/router');
const router = createRouter();
module.exports = router;

router.post('workflows/:workflow/jobs', function(req, res) {
  const workflowKey = req.pathParams.workflow;
  const workflow = documents.getWorkflow(workflowKey, res);
  const job = documents.addJob(req.body, workflow);
  res.send(job);
})
    .pathParam('workflow', joi.string().required(), 'Workflow key')
    .body(schemas.job)
    .response(schemas.jobsResponse)
    .summary('Store a job in the database.')
    .description('Store a job in the database.');

router.post('workflows/:workflow/bulk_jobs', function(req, res) {
  const workflowKey = req.pathParams.workflow;
  const workflow = documents.getWorkflow(workflowKey, res);
  res.send({items: documents.addJobs(req.body.jobs, workflow)});
})
    .pathParam('workflow', joi.string().required(), 'Workflow key')
    .body(schemas.jobs)
    .response(schemas.jobsResponse)
    .summary('Store jobs in bulk in the database.')
    .description('Store jobs in bulk in the database. Recommended max job count of 10,000.');

router.get('workflows/:workflow/job_keys', function(req, res) {
  const workflowKey = req.pathParams.workflow;
  const workflow = documents.getWorkflow(workflowKey, res);
  const jobs = config.getWorkflowCollection(workflow, 'jobs');
  const keys = [];
  for (const job of jobs.all()) {
    keys.push(job._key);
  }
  res.send({items: keys});
})
    .pathParam('workflow', joi.string().required(), 'Workflow key')
    .response(joi.object())
    .summary('Retrieve all job keys for a workflow.')
    .description('Retrieves all job keys from the "jobs" collection for a workflow.');

router.get('/workflows/:workflow/jobs/find_by_status/:status', function(req, res) {
  const workflowKey = req.pathParams.workflow;
  const workflow = documents.getWorkflow(workflowKey, res);
  const jobs = config.getWorkflowCollection(workflow, 'jobs');
  const status = req.pathParams.status;
  const qp = req.queryParams;
  const limit = utils.getItemsLimit(qp.limit);
  try {
    const cursor = jobs.byExample({status: status});
    const items = [];
    for (const job of cursor.skip(qp.skip).limit(limit)) {
      items.push(job);
    }
    res.send(utils.makeCursorResult(items, qp.skip, cursor.count()));
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Get jobs find_by_status status=${status}`);
  }
})
    .pathParam('workflow', joi.string().required(), 'Workflow key')
    .pathParam('status', joi.string().required(), 'Job status.')
    .queryParam('skip', joi.number().default(0))
    .queryParam('limit', joi.number().default(MAX_TRANSFER_RECORDS))
    .response(schemas.batchJobs)
    .summary('Retrieve all jobs with a specific status')
    .description('Retrieves all jobs from the "jobs" collection with a specific status.');

router.get('/workflows/:workflow/jobs/find_by_needs_file/:key', function(req, res) {
  const workflowKey = req.pathParams.workflow;
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(workflowKey, res);
  const files = config.getWorkflowCollection(workflow, 'files');
  if (!files.exists(key)) {
    res.throw(404, `File ${key} is not stored`);
  }
  const file = documents.getWorkflowDocument(workflow, 'files', key, res);
  const qp = req.queryParams;
  const limit = utils.getItemsLimit(qp.limit);
  try {
    const cursor = query.getJobsThatNeedFile(file, workflow, qp.skip, limit);
    res.send(utils.makeCursorResultFromIteration(cursor, qp.skip, cursor.count()));
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Get jobs find_by_needs_file key=${key}`);
  }
})
    .pathParam('workflow', joi.string().required(), 'Workflow key')
    .pathParam('key', joi.string().required(), 'File key')
    .queryParam('skip', joi.number().default(0))
    .queryParam('limit', joi.number().default(MAX_TRANSFER_RECORDS))
    .response(schemas.batchJobs)
    .summary('Retrieve all jobs that need a file')
    .description('Retrieves all jobs connected to a file by the needs edge.');

router.get('/workflows/:workflow/jobs/:key/resource_requirements', function(req, res) {
  const workflowKey = req.pathParams.workflow;
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(workflowKey, res);
  const doc = documents.getWorkflowDocument(workflow, 'jobs', key, res);
  try {
    const rr = query.getJobResourceRequirements(doc, workflow);
    res.send(rr);
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Get jobs resource_requirements key=${key}`);
  }
})
    .pathParam('workflow', joi.string().required(), 'Workflow key')
    .pathParam('key', joi.string().required(), 'Job key')
    .response(schemas.resourceRequirements, 'Resource requirements for the job.')
    .summary('Retrieve the resource requirements for a job.')
    .description('Retrieve the resource requirements for a job by its key.');

router.put('/workflows/:workflow/jobs/:key/resource_requirements/:rr_key', function(req, res) {
  const workflowKey = req.pathParams.workflow;
  const key = req.pathParams.key;
  const rrKey = req.pathParams.rr_key;
  const workflow = documents.getWorkflow(workflowKey, res);
  const job = documents.getWorkflowDocument(workflow, 'jobs', key, res);
  const rr = documents.getWorkflowDocument(workflow, 'resource_requirements', rrKey, res);
  try {
    const edge = query.setOrReplaceJobResourceRequirements(job, rr, workflow);
    res.send(edge);
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Update jobs resource_requirements key=${key}`);
  }
})
    .pathParam('workflow', joi.string().required(), 'Workflow key')
    .pathParam('key', joi.string().required(), 'Job key')
    .body(joi.object().optional(), '')
    .response(schemas.edge, 'Requires edge that connects the job and resource requirements.')
    .summary('Set the resource requirements for a job.')
    .description('Set the resource requirements for a job, replacing any current value.');

router.get('/workflows/:workflow/jobs/:key/process_stats', function(req, res) {
  const workflowKey = req.pathParams.workflow;
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(workflowKey, res);
  const doc = documents.getWorkflowDocument(workflow, 'jobs', key, res);
  try {
    const result = query.listJobProcessStats(doc, workflow);
    res.send(result);
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Update jobs process_stats key=${key}`);
  }
})
    .pathParam('workflow', joi.string().required(), 'Workflow key')
    .pathParam('key', joi.string().required(), 'Job key')
    .response(joi.array().items(schemas.jobProcessStats), 'Process stats for the job.')
    .summary('Retrieve the job process stats for a job.')
    .description('Retrieve the job process stats for a job by its key.');

router.put('/workflows/:workflow/jobs/:key/start_job/:rev/:run_id/:compute_node_key',
    function(req, res) {
      const workflowKey = req.pathParams.workflow;
      const key = req.pathParams.key;
      const rev = req.pathParams.rev;
      const runId = req.pathParams.run_id;
      const workflow = documents.getWorkflow(workflowKey, res);
      const job = documents.getWorkflowDocument(workflow, 'jobs', key, res);
      const computeNode = documents.getWorkflowDocument(
          workflow,
          'compute_nodes',
          req.pathParams.compute_node_key,
      );
      const executedCollection = config.getWorkflowCollection(workflow, 'executed');
      if (job.status != JobStatus.SubmittedPending) {
        res.throw(400, `job status must be ${JobStatus.SubmittedPending}: ${job.status}`);
      }
      if (job._rev != rev) {
        res.throw(409, `Revision conflict for ${job._id}: _rev=${job._rev}`);
      }

      job.status = JobStatus.Submitted;
      try {
        const updatedJob = query.manageJobStatusChange(job, workflow, runId);
        const edge = {_from: computeNode._id, _to: job._id, data: {run_id: runId}};
        executedCollection.save(edge);
        const event = {
          'timestamp': Date.now(),
          'category': 'job',
          'type': 'start',
          'job_key': job._key,
          'job_name': job.name,
          'node_name': computeNode.hostname,
          'message': `Started job ${job._key}`,
        };
        documents.addWorkflowDocument(event, 'events', workflow, false, false);
        res.send(updatedJob);
      } catch (e) {
        utils.handleArangoApiErrors(e, res, `start_job key=${key}`);
      }
    })
    .pathParam('workflow', joi.string().required(), 'Workflow key')
    .pathParam('key', joi.string().required(), 'Job key')
    .pathParam('rev', joi.string().required(), 'Current job revision.')
    .pathParam('run_id', joi.number().integer().required(), 'Current job run ID')
    .pathParam('compute_node_key', joi.string().required(), 'Compute node key')
    .body(joi.object().optional(), '')
    .response(schemas.job, 'Updated job.')
    .summary('Start a job.')
    .description('Start a job and manage side effects.');

router.post('/workflows/:workflow/jobs/:key/complete_job/:status/:rev/:run_id/:compute_node_key',
    function(req, res) {
      const workflowKey = req.pathParams.workflow;
      const key = req.pathParams.key;
      const status = req.pathParams.status;
      const rev = req.pathParams.rev;
      const runId = req.pathParams.run_id;
      const result = req.body;
      const workflow = documents.getWorkflow(workflowKey, res);
      const computeNode = documents.getWorkflowDocument(
          workflow,
          'compute_nodes',
          req.pathParams.compute_node_key,
      );
      if (!query.isJobStatusComplete(status)) {
        res.throw(400, `status=${status} does not indicate completion`);
      }
      const job = documents.getWorkflowDocument(workflow, 'jobs', key, res);
      if (job._rev != rev) {
        res.throw(409, `Revision conflict for ${job._id}: _rev=${job._rev}`);
      }

      job.status = status;
      try {
        const meta = documents.addResult(result, workflow);
        Object.assign(result, meta);

        const returned = config.getWorkflowCollection(workflow, 'returned');
        returned.save({_from: job._id, _to: result._id});
        const updatedJob = query.manageJobStatusChange(job, workflow, runId);
        updatedJob.internal.hash = documents.computeJobInputHash(updatedJob, workflow);
        documents.updateWorkflowDocument(workflow, 'jobs', updatedJob);
        const event = {
          'timestamp': Date.now(),
          'category': 'job',
          'type': 'complete',
          'job_key': job._key,
          'job_name': job.name,
          'node_name': computeNode.hostname,
          'message': `Completed job ${job._key} with status=${status} ` +
              `return_code=${result.return_code}`,
        };
        documents.addWorkflowDocument(event, 'events', workflow, false, false);
        res.send(updatedJob);
      } catch (e) {
        utils.handleArangoApiErrors(e, res, `Complete job key=${key}`);
      }
    })
    .pathParam('workflow', joi.string().required(), 'Workflow key')
    .pathParam('key', joi.string().required(), 'Job key')
    .pathParam('status', joi.string().required(), 'New job status.')
    .pathParam('rev', joi.string().required(), 'Current job revision.')
    .pathParam('run_id', joi.number().integer().required(), 'Current job run ID')
    .pathParam('compute_node_key', joi.string().required(), 'Compute node key')
    .body(schemas.result, 'Result of the job.')
    .response(schemas.job, 'job completed in the collection.')
    .summary('Complete a job and add a result.')
    .description('Complete a job, connect it to a result, and manage side effects.');

router.put('/workflows/:workflow/jobs/:key/manage_status_change/:status/:rev/:run_id',
    function(req, res) {
      const workflowKey = req.pathParams.workflow;
      const key = req.pathParams.key;
      const status = req.pathParams.status;
      const rev = req.pathParams.rev;
      const runId = req.pathParams.run_id;
      const workflow = documents.getWorkflow(workflowKey, res);
      if (query.isJobStatusComplete(status)) {
        res.throw(400, `status=${status} indicates completion. Post complete_job status instead.`);
        return;
      }
      const job = documents.getWorkflowDocument(workflow, 'jobs', key, res);
      if (job._rev != rev) {
        res.throw(400, `Revision conflict for ${job._id}: _rev=${job._rev}`);
        return;
      }
      job.status = status;
      try {
        const updatedJob = query.manageJobStatusChange(job, workflow, runId);
        res.send(updatedJob);
      } catch (e) {
        utils.handleArangoApiErrors(e, res, `Put jobs manage_status_change key=${key}`);
      }
    })
    .pathParam('workflow', joi.string().required(), 'Workflow key')
    .pathParam('key', joi.string().required(), 'Job key')
    .pathParam('status', joi.string().required(), 'New job status')
    .pathParam('rev', joi.string().required(), 'Current job revision')
    .pathParam('run_id', joi.number().integer().required(), 'Current job run ID')
    .body(joi.object().optional(), '')
    .response(schemas.job, 'Updated job.')
    .summary('Change the status of a job and manage side effects.')
    .description('Change the status of a job and manage side effects.');

router.post('/workflows/:workflow/jobs/:key/user_data', function(req, res) {
  const workflowKey = req.pathParams.workflow;
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(workflowKey, res);
  const job = documents.getWorkflowDocument(workflow, 'jobs', key, res);
  const userData = req.body;
  try {
    const doc = documents.addUserData(userData, workflow);
    const stores = config.getWorkflowCollection(workflow, 'stores');
    stores.save({_from: job._id, _to: doc._id});
    res.send(doc);
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Post jobs user_data key=${key}`);
  }
})
    .pathParam('workflow', joi.string().required(), 'Workflow key')
    .pathParam('key', joi.string().required(), 'Job key')
    .body(schemas.userData, 'User data for the job.')
    .response(schemas.userData, 'Database information for the user data.')
    .summary('Store user data for a job.')
    .description('Store user data for a job and connect the two vertexes.');

router.get('/workflows/:workflow/jobs/:key/user_data_stores', function(req, res) {
  const workflowKey = req.pathParams.workflow;
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(workflowKey, res);
  const job = documents.getWorkflowDocument(workflow, 'jobs', key, res);
  try {
    // Shouldn't need skip and limit, but that could be added.
    const items = query.listUserDataStoredByJob(job, workflow).toArray();
    if (items.length > MAX_TRANSFER_RECORDS) {
      throw new Error(`Bug: unhandled case where length of items is too big: ${items.length}`);
    }
    res.send(utils.makeCursorResult(items, 0, items.length));
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Get jobs user_data stored key=${key}`);
  }
})
    .pathParam('workflow', joi.string().required(), 'Workflow key')
    .pathParam('key', joi.string().required(), 'Job key')
    .response(schemas.batchUserData, 'All user data stored for the job.')
    .summary('Retrieve all user data stored for a job.')
    .description('Retrieve all user data stored for a job.');

router.get('/workflows/:workflow/jobs/:key/user_data_consumes', function(req, res) {
  const workflowKey = req.pathParams.workflow;
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(workflowKey, res);
  const job = documents.getWorkflowDocument(workflow, 'jobs', key, res);
  try {
    // Shouldn't need skip and limit, but that could be added.
    const items = query.listUserDataConsumedByJob(job, workflow).toArray();
    if (items.length > MAX_TRANSFER_RECORDS) {
      throw new Error(`Bug: unhandled case where length of items is too big: ${items.length}`);
    }
    res.send(utils.makeCursorResult(items, 0, items.length));
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Get jobs user_data consumed key=${key}`);
  }
})
    .pathParam('workflow', joi.string().required(), 'Workflow key')
    .pathParam('key', joi.string().required(), 'Job key')
    .response(schemas.batchUserData, 'All user data consumed by the job.')
    .summary('Retrieve all user data consumed by a job.')
    .description('Retrieve all user data consumed by a job.');
