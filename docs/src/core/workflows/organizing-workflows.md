# Organizing and Managing Workflows

Workflows can be organized and managed using the `project` and `metadata` fields, which help you
categorize, track, and manage workflows across your organization.

## Overview

- **`project`**: A string field for grouping related workflows (e.g., by department, client, or
  initiative)
- **`metadata`**: A JSON string field for storing arbitrary custom information about the workflow

Both fields are optional and can be set when creating a workflow or updated at any time.

## Using the Project Field

The `project` field is useful for grouping related workflows together. Common use cases:

### By Department or Team

```yaml
name: "data_validation"
project: "data-engineering"
jobs:
  - name: "validate"
    command: "python validate.py"
```

### By Client or Customer

```yaml
name: "client_report_generation"
project: "acme-corp"
description: "Monthly analytics reports for ACME Corp"
jobs:
  - name: "extract"
    command: "python extract_data.py"
  - name: "report"
    command: "python generate_report.py"
    depends_on: ["extract"]
```

### By Application or Service

```yaml
name: "model_retraining"
project: "recommendation-engine"
jobs:
  - name: "collect_data"
    command: "python collect_training_data.py"
  - name: "train"
    command: "python train_model.py"
    depends_on: ["collect_data"]
```

## Using the Metadata Field

The `metadata` field stores arbitrary JSON data, making it perfect for tracking additional context
about your workflow.

### Environment and Version Tracking

```yaml
name: "ml_pipeline"
project: "ai-models"
metadata: '{"environment":"production","model_version":"2.1.0","framework":"pytorch"}'
jobs:
  - name: "preprocess"
    command: "python preprocess.py"
```

### Cost and Billing Tracking

```yaml
name: "data_processing"
project: "analytics-platform"
metadata: '{"cost_center":"eng-data","billing_code":"proj-2024-001","budget":"$5000"}'
jobs:
  - name: "process"
    command: "python process_data.py"
```

### Ownership and Scheduling Information

```yaml
name: "hourly_sync"
project: "data-platform"
metadata: |
  {
    "owner": "data-platform-team",
    "schedule": "0 * * * *",
    "critical": true,
    "on_call": "oncall@company.com",
    "sla": "15 minutes"
  }
jobs:
  - name: "sync"
    command: "python sync_data.py"
```

### Testing and Deployment Metadata

```yaml
name: "integration_tests"
project: "backend-api"
metadata: |
  {
    "test_type": "integration",
    "test_suite": "api_v2",
    "required_before_deploy": true,
    "notifications": ["slack:devops", "email:qa@company.com"],
    "timeout_minutes": 30
  }
jobs:
  - name: "run_tests"
    command: "pytest tests/integration/"
```

## Querying and Filtering Workflows

### Listing Workflows with Project and Metadata

```bash
# View all workflows with their project and metadata
torc workflows list

# View in JSON format for programmatic access
torc workflows list -f json

# Get details of a specific workflow
torc workflows get <workflow_id> -f json
```

### JSON Output Example

```json
{
  "id": 42,
  "name": "ml_training_pipeline",
  "project": "customer-ai",
  "metadata": {
    "environment": "staging",
    "version": "1.2.0",
    "owner": "ml-team"
  },
  "description": "Train and evaluate models",
  "user": "alice",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

## Updating Workflow Organization

You can update the `project` and `metadata` fields at any time:

### Update Project

```bash
# Change workflow project
torc workflows update <workflow_id> --project "new-department"
```

### Update Metadata

```bash
# Update metadata with new information
torc workflows update <workflow_id> --metadata '{"stage":"production","last_review":"2024-01-15"}'

# Or add additional metadata
torc workflows update <workflow_id> --metadata '{"owner":"new-team","priority":"high"}'
```

### Update Both

```bash
torc workflows update <workflow_id> --project "sales" --metadata '{"client":"xyz-corp","contract":"2024-001"}'
```

## Workflow Organization Best Practices

### 1. Consistent Project Naming

Use consistent naming conventions for projects to make filtering easier:

```yaml
# Good: lowercase, dash-separated
project: "customer-data-pipeline"
project: "ml-model-training"
project: "backend-api-tests"

# Avoid: inconsistent naming
project: "Customer Data Pipeline"
project: "ML_Model_Training"
project: "bApiT"
```

### 2. Structured Metadata

Store metadata as valid JSON objects for better programmatic access:

```yaml
# Good: valid JSON object
metadata: '{"owner":"team","env":"prod","version":"1.0"}'

# Avoid: unstructured strings
metadata: 'owner=team, env=prod, version 1.0'
```

### 3. Document Metadata Schema

Keep documentation of what metadata fields your organization uses:

```yaml
metadata: |
  {
    "team": "string - owning team",
    "environment": "string - deployment environment",
    "version": "semver - software version",
    "critical": "boolean - mission-critical",
    "review_date": "ISO8601 - last review date"
  }
```

### 4. Combine Project and Metadata

Use project for high-level categorization and metadata for detailed information:

```yaml
name: "customer_analytics"
project: "analytics-platform"  # High-level grouping
metadata: |                     # Detailed information
  {
    "customer_id": "ACME-2024",
    "region": "us-west",
    "sla": "99.9%",
    "cost_center": "analytics"
  }
```

## Examples

### Data Engineering Workflow

```yaml
name: "daily_etl_pipeline"
project: "data-warehouse"
metadata: |
  {
    "schedule": "daily",
    "run_time": "02:00 UTC",
    "owner": "data-engineering-team",
    "estimated_duration_minutes": 45,
    "source_systems": ["salesforce", "zendesk"],
    "target_table": "customer_analytics",
    "sla_hours": 4
  }
description: "Daily ETL process for customer analytics data"
jobs:
  - name: "extract_salesforce"
    command: "python extract_salesforce.py"
  - name: "extract_zendesk"
    command: "python extract_zendesk.py"
  - name: "transform_and_load"
    command: "python transform_load.py"
    depends_on: ["extract_salesforce", "extract_zendesk"]
```

### Machine Learning Workflow

```yaml
name: "model_training_pipeline"
project: "recommendation-engine"
metadata: |
  {
    "model_type": "collaborative_filtering",
    "framework": "pytorch",
    "training_date": "2024-01-15",
    "dataset_version": "v3.2.0",
    "hyperparameters": {
      "learning_rate": 0.001,
      "batch_size": 64,
      "epochs": 100
    },
    "metrics": {
      "accuracy": 0.94,
      "precision": 0.91,
      "recall": 0.93
    },
    "next_review": "2024-04-15"
  }
description: "Train recommendation models"
jobs:
  - name: "prepare_data"
    command: "python prepare_training_data.py"
  - name: "train_model"
    command: "python train.py"
    depends_on: ["prepare_data"]
  - name: "evaluate"
    command: "python evaluate.py"
    depends_on: ["train_model"]
```

### Testing and CI/CD Workflow

```yaml
name: "backend_integration_tests"
project: "backend-api"
metadata: |
  {
    "ci_cd_integration": true,
    "test_framework": "pytest",
    "coverage_requirement": 85,
    "notifications": {
      "on_failure": ["slack:devops"],
      "on_success": ["slack:dev-team"]
    },
    "blocking_deployment": true,
    "timeout_minutes": 30,
    "retry_count": 2
  }
description: "Run integration tests for backend API"
jobs:
  - name: "setup_test_db"
    command: "python setup_test_db.py"
  - name: "run_tests"
    command: "pytest tests/integration/ -v"
    depends_on: ["setup_test_db"]
  - name: "generate_coverage_report"
    command: "coverage report --html"
    depends_on: ["run_tests"]
```
