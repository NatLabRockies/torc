# How to Parameterize Jobs with Files

Process multiple input files by combining parameterization with file path templating.

## Basic Pattern

Use a parameter to generate jobs for each file:

```yaml
name: process_files

jobs:
  - name: process_{dataset}
    command: python process.py --input data/{dataset}.csv --output results/{dataset}.json
    parameters:
      dataset: [train, test, validation]
```

This creates 3 jobs:

- `process_train` → processes `data/train.csv`
- `process_test` → processes `data/test.csv`
- `process_validation` → processes `data/validation.csv`

## With File Dependencies

Combine parameterization with explicit file definitions for dependency tracking:

```yaml
name: file_pipeline

files:
  - name: raw_{dataset}
    path: data/{dataset}.csv
  - name: processed_{dataset}
    path: results/{dataset}.json

jobs:
  - name: process_{dataset}
    command: python process.py -i ${files.input.raw_{dataset}} -o ${files.output.processed_{dataset}}
    parameters:
      dataset: [train, test, validation]

  - name: aggregate
    command: python aggregate.py --input results/ --output summary.json
    depends_on:
      - process_{dataset}
    parameters:
      dataset: [train, test, validation]
```

The `aggregate` job automatically waits for all `process_*` jobs to complete.

## Processing Numbered Files

Use range syntax for numbered file sequences:

```yaml
jobs:
  - name: convert_{i:03d}
    command: ffmpeg -i video_{i:03d}.mp4 -o audio_{i:03d}.mp3
    parameters:
      i: "1:100"
```

Creates jobs for `video_001.mp4` through `video_100.mp4`.

## Multi-Dimensional Sweeps

Combine multiple parameters for Cartesian product expansion:

```yaml
jobs:
  - name: analyze_{region}_{year}
    command: python analyze.py --region {region} --year {year} --output results/{region}_{year}.json
    parameters:
      region: [north, south, east, west]
      year: "2020:2024"
```

Creates 20 jobs (4 regions × 5 years).

## See Also

- [Simple Parameterization](../tutorials/simple-params.md) — Basic parameter tutorial
- [Advanced Parameterization](../tutorials/advanced-params.md) — Multi-dimensional sweeps
- [Job Parameterization Reference](../reference/parameterization.md) — Complete syntax
