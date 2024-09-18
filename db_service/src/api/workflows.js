'use strict';
const joi = require('joi');
const db = require('@arangodb').db;
const {MAX_TRANSFER_RECORDS} = require('../defs');
const {JobStatus} = require('../defs');
const utils = require('../utils');
const query = require('../query');
const documents = require('../documents');
const graphs = require('../graphs');
const schemas = require('./schemas');
const createRouter = require('@arangodb/foxx/router');
const router = createRouter();
const collection = db._collection('workflows');
const VERSION = '0.4.2'; // TODO: this needs to be stored in one file for all torc SW to read
module.exports = router;

router.get('/ping', function(req, res) {
  res.send({message: 'torc-service is running'});
})
    .response(joi.object(), 'Message')
    .summary('Check if the service is running.')
    .description('Check if the service is running.');

router.get('/version', function(req, res) {
  res.send({version: VERSION});
})
    .response(joi.object(), 'Version')
    .summary('Return the version of the service.')
    .description('Return the version of the service.');

router.post('/workflows', function(req, res) {
  try {
    const doc = documents.addWorkflow(req.body);
    res.send(doc);
  } catch (e) {
    utils.handleArangoApiErrors(e, res, 'Post new workflow');
  }
})
    .body(schemas.workflow, 'Collection of jobs and dependent resources.')
    .response(schemas.workflow, 'Collection of jobs and dependent resources')
    .summary('Store a workflow.')
    .description('Store a workflow in the "workflows" collection.');

router.put('/workflows/:key', function(req, res) {
  const key = req.pathParams.key;
  const doc = req.body;
  if (key != doc._key) {
    res.throw(400, `key=${key} does not match ${doc._key}`);
  }
  if (doc._rev == null) {
    res.throw(400, `Updating a workflow requires the existing revision`);
  }
  try {
    const meta = collection.update(doc, doc, {mergeObjects: false});
    Object.assign(doc, meta);
    res.send(doc);
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Update workflow key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Key of the workflow.')
    .body(schemas.workflow, 'workflow to update in the collection.')
    .response(schemas.workflow, 'workflow updated in the collection.')
    .summary('Update workflow')
    .description('Update a document in the "workflows" collection.');

router.get('/workflows/:key', function(req, res) {
  res.send(documents.getWorkflow(req.pathParams.key, res));
})
    .pathParam('key', joi.string().required(), 'key of the workflows document')
    .response(schemas.workflow)
    .summary('Retrieve the workflow for an key.')
    .description('Retrieve the document for a key from the "workflows" collection.');

router.get('/workflows', function(req, res) {
  try {
    const qp = req.queryParams;
    const limit = utils.getItemsLimit(qp.limit);
    const example = {};
    for (const filterField of ['description', 'name', 'user', 'is_archived']) {
      if (qp[filterField] != null) {
        example[filterField] = qp[filterField];
      }
    }
    const totalCount = Object.keys(example).length == 0 ? collection.count() :
        collection.byExample(example).count();
    const items = Object.keys(example).length == 0 ?
      collection.all().skip(qp.skip).limit(limit) :
      collection.byExample(example).skip(qp.skip).limit(limit);
    res.send(
        utils.makeCursorResult(
            items.toArray(),
            qp.skip,
            totalCount,
            qp.sort_by,
            qp.reverse_sort,
        ),
    );
  } catch (e) {
    if (e.isArangoError) {
      res.throw(400, `${e}`, e);
    }
    throw e;
  }
})
    .queryParam('skip', joi.number().default(0))
    .queryParam('sort_by', joi.string().default(null))
    .queryParam('reverse_sort', joi.boolean().default(false))
    .queryParam('limit', joi.number().default(MAX_TRANSFER_RECORDS))
    .queryParam('name', joi.string())
    .queryParam('user', joi.string())
    .queryParam('description', joi.string())
    .queryParam('is_archived', joi.boolean())
    .response(schemas.batchWorkflows)
    .summary('Retrieve all workflows')
    .description('Retrieves all documents from the "workflows" collection.');

router.delete('/workflows/:key', function(req, res) {
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(key, res);
  try {
    documents.deleteWorkflow(workflow);
    res.send(workflow);
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Delete workflow key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key.')
    .body(joi.object().optional())
    .response(schemas.workflow, 'workflow stored in the collection.')
    .summary('Delete a workflow')
    .description('Deletes a document from the "workflows" collection by key.');

router.get('/workflows/:key/is_complete', function(req, res) {
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(key, res);
  const status = query.getWorkflowStatus(workflow);
  try {
    res.send({is_canceled: status.is_canceled, is_complete: query.isWorkflowComplete(workflow)});
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Check workflow is_complete key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .response(schemas.isComplete)
    .summary('Report whether the workflow is complete')
    .description('Reports true if all jobs in the workflow are complete.');

router.get('/workflows/:key/ready_job_requirements', function(req, res) {
  const key = req.pathParams.key;
  const schedulerConfigId = req.queryParams.scheduler_config_id;
  const workflow = documents.getWorkflow(key, res);
  try {
    const result = query.getReadyJobRequirements(workflow, schedulerConfigId);
    res.send(result);
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Get ready_job_requirements key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .queryParam('scheduler_config_id', joi.string().optional().allow(null, ''),
        'Limit output to jobs assigned this scheduler.')
    .response(schemas.readyJobsResourceRequirements, 'result')
    .summary('Return the resource requirements for ready jobs.')
    .description(`Return the resource requirements for jobs with a status of ready.`);

router.post('/workflows/:key/initialize_jobs', function(req, res) {
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(key, res);
  if (workflow.is_archived) {
    res.throw(400, `initialized_jobs is not supported on an archived workflow.`);
  }
  try {
    if (req.clear_ephemeral_user_data) {
      query.clearEphemeralUserData(workflow);
    }
    query.addBlocksEdgesFromFiles(workflow);
    query.addBlocksEdgesFromUserData(workflow);
    query.initializeJobStatus(workflow, req.queryParams.only_uninitialized);
    res.send({message: 'Initialized job status'});
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Initialize jobs for workflow key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .queryParam('only_uninitialized', joi.boolean().optional().default(false),
        'Only initialize jobs with a status of uninitialized.')
    .queryParam('clear_ephemeral_user_data', joi.boolean().optional().default(true),
        'Clear all ephemeral user data.')
    .body(joi.object().optional(), '')
    .response(joi.object(), 'message')
    .summary('Initialize job relationships.')
    .description('Initialize job relationships based on file and user_data relationships.');

router.post('/workflows/:key/process_changed_job_inputs', function(req, res) {
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(key, res);
  try {
    query.clearEphemeralUserData(workflow);
    const reinitializedJobs = documents.processChangedJobInputs(workflow);
    res.send({reinitialized_jobs: reinitializedJobs});
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Process changed user data workflow key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .body(joi.object().optional(), '')
    .response(schemas.processChangedJobInputsResponse)
    .summary('Check for changed job inputs and update status accordingly.')
    .description('Check for changed job inputs and update status accordingly.');

router.get('/workflows/:key/missing_user_data', function(req, res) {
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(key, res);
  try {
    const missingUserData = query.listMissingUserData(workflow);
    res.send({user_data: missingUserData});
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `List missing user_data for workflow key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .response(schemas.missingUserDataResponse)
    .summary('List missing user data that should exist.')
    .description('List missing user data that should exist.');

router.get('/workflows/:key/required_existing_files', function(req, res) {
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(key, res);
  try {
    const requiredFiles = query.listRequiredExistingFiles(workflow);
    res.send({files: requiredFiles});
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `List files that must exist for workflow key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .response(schemas.requiredExistingFilesResponse)
    .summary('List files that must exist.')
    .description('List files that must exist.');

router.post('/workflows/:key/prepare_jobs_for_submission', function(req, res) {
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(key, res);
  try {
    const status = query.getWorkflowStatus(workflow);
    if (status.is_canceled) {
      res.send([]);
    } else {
      const resources = req.body;
      const qp = req.queryParams == null ? {} : req.queryParams;
      const reason = {message: ''};
      const jobs = query.prepareJobsForSubmission(
          workflow, resources, qp.sort_method, qp.limit, reason,
      );
      res.send({jobs: jobs, reason: reason.message});
    }
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `prepare_jobs_for_submission workflow key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .queryParam('sort_method', joi.string().default('gpus_runtime_memory')
        .valid('gpus_runtime_memory', 'gpus_memory_runtime', 'none'))
    .queryParam('limit', joi.number().default(MAX_TRANSFER_RECORDS))
    .body(schemas.workerResources, 'Available worker resources.')
    .response(joi.object().required().keys(
        {jobs: joi.array().items(schemas.job), reason: joi.string()}),
    'Jobs that are ready for submission.',
    )
    .summary('Return ready jobs, accounting for resource requirements.')
    .description('Return jobs that are ready for submission and meet worker resource ' +
    'Sets status to submitted_pending.');

router.post('/workflows/:key/prepare_next_jobs_for_submission', function(req, res) {
  const key = req.pathParams.key;
  const limit = req.queryParams.limit;
  const workflow = documents.getWorkflow(key, res);
  try {
    const status = query.getWorkflowStatus(workflow);
    if (status.is_canceled) {
      res.send({jobs: []});
    } else {
      const jobs = query.prepareJobsForSubmissionNoResourceChecks(workflow, limit);
      res.send({jobs: jobs});
    }
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `prepare_jobs_for_submission workflow key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .queryParam('limit', joi.number().default(1))
    .body(joi.object().optional(), '')
    .response(joi.object().required().keys({jobs: joi.array().items(schemas.job)}),
        'Jobs that are ready for submission.',
    )
    .summary('Return user-requested number of ready jobs.')
    .description('Return user-requested number of jobs that are ready for submission. ' +
      'Sets status to submitted_pending.');

router.post('/workflows/:key/prepare_jobs_for_scheduling', function(req, res) {
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(key, res);
  try {
    const status = query.getWorkflowStatus(workflow);
    if (status.is_canceled) {
      res.send({schedulers: []});
    } else {
      res.send({schedulers: query.prepareJobsForScheduling(workflow)});
    }
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `prepare_jobs_for_submission workflow key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .body(joi.object().optional(), '')
    .response(joi.object().required().keys({
      schedulers: joi.array().items(schemas.computeNodeScheduleParams),
    }),
    'Schedulers that need to be activated.',
    )
    .summary('Return scheduler parameters that need to be activated.')
    .description('Return scheduler parameters that need to be activated. Sets job status to scheduled.');

router.post('/workflows/:key/auto_tune_resource_requirements', function(req, res) {
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(key, res);
  try {
    query.setupAutoTuneResourceRequirements(workflow);
    res.send({message: 'Enabled jobs for auto-tune mode.'});
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `auto_tune_resource_requirements workflow key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .body(joi.object().optional(), '')
    .response(joi.object(), 'Message')
    .summary('Enable workflow for auto-tuning resource requirements.')
    .description('Enable workflow for auto-tuning resource requirements.');

router.post('/workflows/:key/process_auto_tune_resource_requirements_results', function(req, res) {
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(key, res);
  try {
    query.processAutoTuneResourceRequirementsResults(workflow);
    res.send({message: 'Processed the results of auto-tuning resource requirements.'});
  } catch (e) {
    const tag = `process_auto_tune_resource_requirements_results workflow key=${key}`;
    utils.handleArangoApiErrors(e, res, tag);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .body(joi.object().optional(), '')
    .response(joi.object(), 'Message')
    .summary('Process the results of auto-tuning resource requirements.')
    .description('Process the results of auto-tuning resource requirements.');

router.get('/workflows/:key/config', function(req, res) {
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(key, res);
  try {
    const config = query.getWorkflowConfig(workflow);
    res.send(config);
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Get workflow config key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .response(schemas.workflowConfig)
    .summary('Returns the workflow config.')
    .description('Returns the workflow config.');

router.put('/workflows/:key/config', function(req, res) {
  const key = req.pathParams.key;
  const config = req.body;
  // Validate that the workflow key is correct.
  documents.getWorkflow(key, res);
  try {
    db.workflow_configs.update(config, config, {mergeObjects: false});
    res.send(config);
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Update workflow config key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .body(schemas.workflowConfig, 'Updated workflow config')
    .response(schemas.workflowConfig)
    .summary('Updates the workflow config.')
    .description('Updates the workflow config.');

router.put('/workflows/:key/cancel', function(req, res) {
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(key, res);
  try {
    documents.cancelWorkflow(workflow);
    res.send({message: `Canceled workflow`});
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Cancel workflow key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .body(joi.object().optional(), '')
    .response(joi.object(), 'message')
    .summary('Cancel workflow.')
    .description(`Cancel workflow. Workers will detect the status change and cancel jobs.`);

router.post('/workflows/:key/reset_status', function(req, res) {
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(key, res);
  if (workflow.is_archived) {
    res.throw(400, `reset_status is not supported on an archived workflow.`);
  }
  try {
    query.resetWorkflowStatus(workflow);
    res.send({message: `Reset workflow status`});
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Reset workflow status workflow key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .body(joi.object().optional(), '')
    .response(joi.object(), 'message')
    .summary('Reset worklow status.')
    .description(`Reset workflow status.`);

router.post('/workflows/:key/reset_job_status', function(req, res) {
  const key = req.pathParams.key;
  const failedOnly = req.queryParams.failed_only;
  const workflow = documents.getWorkflow(key, res);
  if (workflow.is_archived) {
    res.throw(400, `reset_job_status is not supported on an archived workflow.`);
  }
  try {
    if (failedOnly) {
      query.resetFailedJobStatus(workflow);
    } else {
      query.resetJobStatus(workflow);
    }
    const status = JobStatus.Uninitialized;
    res.send({message: `Reset job status to ${status} failed_only=${failedOnly}`});
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Reset job status workflow key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .queryParam('failed_only', joi.boolean().default(false), 'Only reset failed jobs')
    .body(joi.object().optional(), '')
    .response(joi.object(), 'message')
    .summary('Reset job status.')
    .description(`Reset status for jobs to ${JobStatus.Uninitialized}.`);

router.get('/workflows/:key/status', function(req, res) {
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(key, res);
  try {
    const status = query.getWorkflowStatus(workflow);
    res.send(status);
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Get workflow status key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .response(schemas.workflowStatus)
    .summary('Reports the workflow status.')
    .description('Reports the workflow status.');

router.put('/workflows/:key/status', function(req, res) {
  const status = req.body;
  const key = req.pathParams.key;
  // Validate that the workflow key is correct.
  documents.getWorkflow(key, res);
  try {
    db.workflow_statuses.update(status, status, {mergeObjects: false});
    res.send(status);
  } catch (e) {
    utils.handleArangoApiErrors(e, res, `Update workflow status key=${key}`);
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .body(schemas.workflowStatus, 'Updated workflow status')
    .response(schemas.workflowStatus)
    .summary('Reports the workflow status.')
    .description('Reports the workflow status.');

router.get('/workflows/:key/collection_names', function(req, res) {
  const workflow = documents.getWorkflow(req.pathParams.key, res);
  res.send({names: documents.listWorkflowCollectionNames(workflow)});
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .response(joi.object().required().keys({names: joi.array().items(joi.string())}))
    .summary('Retrieve all collection names for one workflow.')
    .description('Retrieve all collection names for one workflow.');

// This is a post because the command needs to accept variable parameters and there
// is no way to define all of those for Swagger. Use a request body for them instead.
router.post('/workflows/:key/join_by_inbound_edge/:collection/:edge', function(req, res) {
  const key = req.pathParams.key;
  const collection = req.pathParams.collection;
  const edge = req.pathParams.edge;
  const qp = req.queryParams;
  const filters = req.body;
  const limit = utils.getItemsLimit(qp.limit);
  const workflow = documents.getWorkflow(key, res);
  try {
    const cursor = query.joinCollectionsByInboundEdge(
        workflow, collection, edge, filters, qp.skip, limit);
    res.send(utils.makeCursorResult(convertItems(cursor), qp.skip, cursor.count()));
  } catch (e) {
    utils.handleArangoApiErrors(e, res, 'Join by inbound edge');
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .pathParam('collection', joi.string().required(), 'From collection')
    .pathParam('edge', joi.string().required(), 'Edge name')
    .queryParam('collection_key', joi.string().optional())
    .queryParam('collection_name', joi.string().optional())
    .queryParam('skip', joi.number().default(0))
    .queryParam('limit', joi.number().default(MAX_TRANSFER_RECORDS))
    .body(joi.object().required(), 'Filters for query')
    .response(schemas.batchObjects)
    .summary('Retrieve a joined table of two collections.')
    .description('Retrieve a table of the collections joined by an inbound edge.');

router.post('/workflows/:key/join_by_outbound_edge/:collection/:edge', function(req, res) {
  const key = req.pathParams.key;
  const collection = req.pathParams.collection;
  const edge = req.pathParams.edge;
  const qp = req.queryParams;
  const filters = req.body;
  const limit = utils.getItemsLimit(qp.limit);
  const workflow = documents.getWorkflow(key, res);
  try {
    const cursor = query.joinCollectionsByOutboundEdge(
        workflow, collection, edge, filters, qp.skip, limit);
    res.send(utils.makeCursorResult(convertItems(cursor), qp.skip, cursor.count()));
  } catch (e) {
    utils.handleArangoApiErrors(e, res, 'Join by outbound edge');
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .pathParam('collection', joi.string().required(), 'From collection')
    .pathParam('edge', joi.string().required(), 'Edge name')
    .queryParam('collection_key', joi.string().optional())
    .queryParam('collection_name', joi.string().optional())
    .queryParam('skip', joi.number().default(0))
    .queryParam('limit', joi.number().default(MAX_TRANSFER_RECORDS))
    .body(joi.object().required(), 'Filters for query')
    .response(schemas.batchObjects)
    .summary('Retrieve a joined table of two collections.')
    .description('Retrieve a table of the collections joined by an outbound edge.');

router.get('/workflows/:key/latest_event_timestamp', function(req, res) {
  const key = req.pathParams.key;
  const workflow = documents.getWorkflow(key, res);
  try {
    res.send({timestamp: query.getLatestEventTimestamp(workflow)});
  } catch (e) {
    utils.handleArangoApiErrors(e, res, 'Get latest event timestamp');
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .response(joi.object())
    .summary('Return the timestamp of the latest event.')
    .description('Return the timestamp of the latest event in ms since the epoch in UTC.');

// This method would be better expressed as a filter on get_events, but we don't currently
// have a way to say "greater than this value".
router.get('/workflows/:key/events_after_timestamp/:timestamp', function(req, res) {
  const workflowKey = req.pathParams.key;
  const workflow = documents.getWorkflow(workflowKey, res);
  const qp = req.queryParams;
  try {
    const cursor = query.getEventsAfterTimestamp(
        workflow, req.pathParams.timestamp, qp.category, qp.limit,
    );
    const items = cursor.toArray();
    res.send(utils.makeCursorResult(items, 0, cursor.count()));
  } catch (e) {
    utils.handleArangoApiErrors(e, res, 'Get events after key');
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .pathParam('timestamp', joi.number().required(),
        'Timestamp expressed as number of milliseconds since the epoch in UTC')
    .queryParam('category', joi.string().default(null))
    .queryParam('skip', joi.number().default(0), 'Ignored')
    .queryParam('limit', joi.number().default(MAX_TRANSFER_RECORDS))
    .response(schemas.batchObjects)
    .summary('Return all events newer than the event with event_key.')
    .description('Return all events newer than the event with event_key.');

router.get('/workflows/:key/dot_graph/:name', function(req, res) {
  const key = req.pathParams.key;
  const name = req.pathParams.name;
  const workflow = documents.getWorkflow(key, res);
  try {
    res.send({graph: graphs.makeDotGraph(workflow, name)});
  } catch (e) {
    utils.handleArangoApiErrors(e, res, 'Make DOT graph');
  }
})
    .pathParam('key', joi.string().required(), 'Workflow key')
    .pathParam('name', joi.string().required(), 'Graph name')
    .response(schemas.dotGraphResponse)
    .summary('Build a string for a DOT graph.')
    .description('Build a string for a DOT graph.');

/**
 * Convert items in a cursor per the normal API rules.
 * @param {Object} cursor
 * @return {Array}
 */
function convertItems(cursor) {
  const items = [];
  for (const item of cursor) {
    if (item.from._id.split('__')[0] == 'jobs') {
      item.from = item.from;
    }
    if (item.to._id.split('__')[0] == 'jobs') {
      item.to = item.to;
    }
    items.push(item);
  }
  return items;
}
