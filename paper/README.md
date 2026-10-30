# Paper Reproducibility

## Generate The PDF

Use the official [Inara](https://github.com/openjournals/inara) JOSS templates, pinned to `v1.3.1`.
Install [Pandoc](https://pandoc.org/installing.html), a TeX distribution with `latexmk` and
`lualatex`, and the [Hack font](https://sourcefoundry.org/hack/). Make the executables available on
`PATH`. For TinyTeX, include its `bin/x86_64-linux` directory on `PATH`; the tested setup used
TinyTeX `v2026.10` with these additional packages:

```bash
tlmgr update --self
tlmgr install marginnote titlesec pgf hyperxmp caption orcidlink tcolorbox seqsplit \
  xstring fancyhdr fontsetup newcomputermodern draftwatermark lineno preprint \
  environ trimspaces xkeyval luatex85 selnolig pdfx luacode luatexbase colorprofiles
```

Obtain the templates once:

```bash
git clone --depth 1 --branch v1.3.1 https://github.com/openjournals/inara.git \
  "$HOME/inara-v1.3.1"
```

From the Torc repository root, run:

```bash
repo="$PWD"
inara="$HOME/inara-v1.3.1"
(
  cd "$inara" || exit 1
  pandoc --data-dir=data --defaults=shared --defaults=pdf \
    --defaults=resources/joss/defaults.yaml \
    --resource-path=".:resources:$repo/paper" \
    --variable=joss --variable=draft: --metadata=draft:true \
    --metadata=software_repository_url:https://github.com/NatLabRockies/torc \
    --output="$repo/paper/paper-joss.pdf" "$repo/paper/paper.md" --quiet
)
```

This writes `paper/paper-joss.pdf` using `paper/paper.md`, `paper/paper.bib`,
and `paper/architecture.png`. The empty `--variable=draft:` disables the watermark and line
numbers, while `--metadata=draft:true` retains unpublished status and placeholder publication
metadata. This is a JOSS-formatted preview, not a published article. To restore the watermark and
line numbers for review, use `--variable=draft:true` instead.

The native build was tested locally. The older `paper/paper.pdf` and `pdf-authors.lua` belong to
the previous generic Pandoc/Tectonic preview and are not used by this build. Regenerate the PDF
whenever the manuscript or its inputs change.

## Local Diamond Workflow

Use the existing `examples/yaml/diamond_workflow.yaml` as a small reviewer entry point. Four jobs
prepare data, process two independent branches, and combine the results. Dependencies are inferred
from declared file inputs and outputs rather than explicit job ordering. This is an orchestration
demonstration, not a scientific benchmark or a demonstration of every Torc feature.

Requirements: a repository checkout, Bash, Python 3 for the output check, and matching `torc` and
`torc-server` binaries on `PATH`. See the repository's installation instructions. No Slurm
allocation, GPU, external database service, or Python packages are required.

**Verification status:** These commands have been checked against the existing example and CLI
documentation, but have not been run end to end in the paper-editing environment, which lacks Torc
binaries and Cargo. Complete that check against the release selected for review.

Run from the repository root in Bash. The temporary working directory isolates the input, generated
files, database, and logs from existing workflows. Keep this shell open for the later commands.

```bash
repo="$PWD"
scratch=$(mktemp -d)
mkdir -p "$scratch/tests/scripts"
cp "$repo/tests/scripts/"{preprocess,work,postprocess}.sh "$scratch/tests/scripts/"
printf '%s\n' '{"data": "initial input", "value": 42}' > "$scratch/f1.json"
cd "$scratch"

torc -s --db "$scratch/torc.db" run "$repo/examples/yaml/diamond_workflow.yaml" \
  --max-parallel-jobs 2 --output-dir "$scratch/logs"
```

The graph permits `work1` and `work2` to run independently after `preprocess`; `postprocess` waits
for both branches. These commands are short, so overlapping execution is not guaranteed.

## Inspect And Check

Each standalone command starts a temporary local server. The following commands reopen the same
database after execution has stopped, demonstrating retained state. The workflow ID is `1` in this
fresh database.

```bash
torc -s --db "$scratch/torc.db" status 1
torc -s --db "$scratch/torc.db" results list 1
```

Expect four completed jobs and four results with return code zero. Check these records rather than
relying on the exit code of `torc run`, which can return zero even when individual jobs fail. The
generated files are `f2.json` through `f6.json`; `f6.json` contains both processed branches.

```bash
python3 - <<'PY'
import json

with open("f6.json") as stream:
    result = json.load(stream)
assert result["source"] == "postprocess", result
assert len(result["inputs"]) == 2, result
for branch in result["inputs"]:
    assert branch["source"] == "work", branch
    assert branch["processed"]["source"] == "preprocess", branch
    assert branch["processed"]["input"] == {"data": "initial input", "value": 42}, branch
print("Verified both diamond branches and the final output.")
PY
```

Keep the database together with the generated files and `logs/` for inspection. Database persistence
does not archive the external files. Return to the checkout with `cd "$repo"` when finished.

## Further Examples

The existing documentation covers resource monitoring, parameter expansion, selective reruns, and
Slurm execution separately. Keeping the reviewer entry point small avoids requiring cluster access
or substantial compute resources just to verify the basic workflow lifecycle.
