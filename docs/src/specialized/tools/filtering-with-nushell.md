# Tutorial 11: Filtering CLI Output with Nushell

This tutorial teaches you how to filter and analyze Torc CLI output using
[Nushell](https://www.nushell.sh/), a modern shell with powerful structured data capabilities.

## Learning Objectives

By the end of this tutorial, you will:

- Understand why Nushell is useful for filtering Torc output
- Know how to filter jobs by status, name, and other fields
- Be able to analyze results and find failures quickly
- Create complex queries combining multiple conditions

## Prerequisites

- Torc CLI installed and configured
- A workflow with jobs (ideally one with various statuses)

## Why Nushell?

Torc's CLI can output JSON with the `-f json` flag. While tools like `jq` can process JSON, Nushell
offers a more readable, SQL-like syntax that's easier to learn and use interactively.

Compare filtering failed jobs:

```bash
# jq (cryptic syntax)
torc jobs list 123 -f json | jq '.items[] | select(.status == "failed")'

# Nushell (readable, SQL-like)
torc jobs list 123 -f json | from json | get items | where status == "failed"
```

Nushell is:

- **Cross-platform**: Works on Linux, macOS, and Windows
- **Readable**: Uses intuitive commands like `where`, `select`, `sort-by`
- **Interactive**: Tab completion and helpful error messages
- **Powerful**: Built-in support for JSON, YAML, CSV, and more

## Installing Nushell

Install Nushell from [nushell.sh/book/installation](https://www.nushell.sh/book/installation.html):

```bash
# macOS
brew install nushell

# Windows
winget install nushell

# Linux (various methods available)
cargo install nu
```

After installation, run `nu` to start a Nushell session. You can use Nushell interactively or run
individual commands with `nu -c "command"`.

## Basic Filtering

### Setup: Get JSON Output

All examples assume you have a workflow ID. Replace `$WORKFLOW_ID` with your actual ID:

```nu
# In Nushell, set your workflow ID
let wf = 123
```

### List All Jobs

```nu
torc jobs list $wf -f json | from json | get items
```

This parses the JSON and extracts the `jobs` array into a table.

### Filter by Status

Find all failed jobs:

```nu
torc jobs list $wf -f json | from json | get items | where status == "failed"
```

Find jobs that are ready or running:

```nu
torc jobs list $wf -f json | from json | get items | where status in ["ready", "running"]
```

### Filter by Name Pattern

Find jobs with "train" in the name:

```nu
torc jobs list $wf -f json | from json | get items | where name =~ "train"
```

The `=~` operator performs substring/regex matching.

### Combine Conditions

Find failed jobs with "process" in the name:

```nu
torc jobs list $wf -f json | from json | get items | where status == "failed" and name =~ "process"
```

Find jobs that failed or were canceled:

```nu
torc jobs list $wf -f json | from json | get items | where status == "failed" or status == "canceled"
```

## Selecting and Formatting Output

### Select Specific Columns

Show only name and status:

```nu
torc jobs list $wf -f json | from json | get items | select name status
```

### Sort Results

Sort by name:

```nu
torc jobs list $wf -f json | from json | get items | sort-by name
```

Sort failed jobs by ID (descending):

```nu
torc jobs list $wf -f json | from json | get items | where status == "failed" | sort-by id -r
```

### Count Results

Count jobs by status:

```nu
torc jobs list $wf -f json | from json | get items | group-by status | transpose status jobs | each { |row| { status: $row.status, count: ($row.jobs | length) } }
```

Or more simply, count failed jobs:

```nu
torc jobs list $wf -f json | from json | get items | where status == "failed" | length
```

## Analyzing Results

### Find Jobs with Non-Zero Return Codes

```nu
torc results list $wf -f json | from json | get items | where return_code != 0
```

### Find Results with Specific Errors

```nu
torc results list $wf -f json | from json | get items | where return_code != 0 | select job_id return_code
```

### Join Jobs with Results

Get job names for failed results:

```nu
let jobs = (torc jobs list $wf -f json | from json | get items)
let results = (torc results list $wf -f json | from json | get items | where return_code != 0)
$results | each { |r|
    let job = ($jobs | where id == $r.job_id | first)
    { name: $job.name, return_code: $r.return_code, job_id: $r.job_id }
}
```

## Working with User Data

### List User Data Entries

```nu
torc user-data list $wf -f json | from json | get items
```

### Filter by Key

Find user data with a specific key:

```nu
torc user-data list $wf -f json | from json | get items | where key =~ "config"
```

### Parse JSON Values

User data values are JSON strings. Parse and filter them:

```nu
torc user-data list $wf -f json | from json | get items | each { |ud|
    { key: $ud.key, value: ($ud.value | from json) }
}
```

## Practical Examples

### Example 1: Debug Failed Jobs

Find failed jobs and get their result details:

```nu
# Get failed job IDs
let failed_ids = (torc jobs list $wf -f json | from json | get items | where status == "failed" | get id)

# Show results for those jobs
torc results list $wf -f json | from json | get items | where job_id in $failed_ids | select job_id return_code
```

### Example 2: Find Stuck Jobs

Find jobs that have been running for a long time (status is "running"):

```nu
torc jobs list $wf -f json | from json | get items | where status == "running" | select id name
```

### Example 3: Parameter Sweep Analysis

For a parameterized workflow, find which parameter values failed:

```nu
torc jobs list $wf -f json | from json | get items | where status == "failed" and name =~ "lr" | get name
```

### Example 4: Export to CSV

Export failed jobs to CSV for further analysis:

```nu
torc jobs list $wf -f json | from json | get items | where status == "failed" | to csv | save failed_jobs.csv
```

## Quick Reference

| Operation           | Nushell Command                            |
| ------------------- | ------------------------------------------ |
| Parse JSON          | `from json`                                |
| Get field           | `get items`                                |
| Filter rows         | `where status == "failed"`                 |
| Select columns      | `select name status id`                    |
| Sort                | `sort-by name`                             |
| Sort descending     | `sort-by id -r`                            |
| Count               | `length`                                   |
| Substring match     | `where name =~ "pattern"`                  |
| Multiple conditions | `where status == "failed" and name =~ "x"` |
| In list             | `where status in ["ready", "running"]`     |
| Group by            | `group-by status`                          |
| Save to file        | `save output.json`                         |
| Convert to CSV      | `to csv`                                   |

## Tips

1. **Use `nu` interactively**: Start a Nushell session to explore data step by step
2. **Tab completion**: Nushell provides completions for commands and field names
3. **Pipeline debugging**: Add `| first 5` to see a sample before processing all data
4. **Save queries**: Create shell aliases or scripts for common filters

## What You Learned

In this tutorial, you learned:

- Why Nushell is a great tool for filtering Torc CLI output
- How to filter jobs by status and name patterns
- How to analyze results and find failures
- How to work with user data
- Practical examples for debugging workflows

## Next Steps

- [Nushell Documentation](https://www.nushell.sh/book/) - Learn more about Nushell's capabilities
- [Torc CLI Reference](../../core/reference/cli.md) - Full list of CLI commands and their JSON
  output
