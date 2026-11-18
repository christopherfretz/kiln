# Releasing kiln

How a kiln release is cut, from the freeze to the announcement. The gate
that every tag needs is one command, `cargo xtask release-gate`
(contract: [../spec/harness.md](../spec/harness.md) "Release gate"); this
page is the procedure around it. The records of 1.0.0 to 1.2.1 in
[status/](status/) were typed by hand from hand-written gate scripts;
at the 1.2.0 and 1.2.1 gates three OSS rows (`llvm_full`, `llvm_clang`,
`llvm_selfhost`) were carried over from 1.0.0 instead of re-run, and a
regression in a directory only `llvm_full` builds shipped. From 1.3.0 on
the record is written by `release-gate record` and checked by
`release-gate check`, and a row that was not run at the release
candidate cannot appear in it.

## 1. Freeze

Nothing new lands on `main` for the release; fixes found by the gate
land as ordinary pull requests and move the release candidate (RC) to
the new head. Every gate result is tied to one commit: a new RC makes
every earlier result stale, and the gate re-runs it.

## 2. Version and CHANGELOG

One pull request bumps `[workspace.package] version` in `Cargo.toml` and
turns CHANGELOG.md's "Planned as X.Y.Z" section into the release's
section. Its merge commit is the RC.

## 3. The gate on the RC

On the aarch64 development host, in a checkout of the RC with no local
changes (`source scripts/env.sh` first):

```sh
cargo xtask release-gate plan                      # what the gate is, what is done
cargo xtask release-gate run --external downstream='<command>'
```

`run` starts with the release archive (`cargo xtask dist --check`,
`sha256sum -c`, the unpacked `kiln++ --version`); every later step uses
that `kiln++`. Then two tracks run side by side on the shared jobserver,
as the hand-driven 1.2.1 gate did: the merge gate (`cargo xtask ci`), the
clang 19 lane, the precompiled-header sweep and the full libc++ suite on
one; gcc.c-torture at gnu23 and gnu17, the full GCC C++ testsuite
(`cargo xtask gxx-dg`, every suite at `-O0` and `-O2`) and every row of
`scripts/oss/projects.toml`, the three LLVM rows included, on the other.
`plan` prints each step's estimate; the manifest's estimates are upper
bounds:

- archive: 30 min, 3 GB
- merge gate: 180 min, 25 GB; clang 19 lane: 60 min, 10 GB
- precompiled-header sweep: 120 min, 90 GB free needed (its own check)
- libc++ (aarch64): 150 min, 15 GB
- gcc.c-torture: 60 min and 2 GB per standard
- g++.dg (aarch64): 120 min, 10 GB
- each OSS row: up to 30 min and 5 GB; the LLVM rows up to 240 min and
  40 GB each (the sources stay in the state directory)

A whole gate took about 7 hours at 1.2.1. The host is shared: `run`
refuses a step whose scratch need would leave less than 50 GB free, and
a harness restart that kills it costs only the step in flight. Run the
same command again: steps with a completed record for this commit,
version and `kiln++` are skipped; a step without one, or with a record
made at another commit, with another `kiln++` or without its counts, is
re-run. There is no way to accept such a record.

The state lives in `<target>/release-gate` (`--state <dir>` to move it):
one record per step, its log under `logs/`, the run's TSVs under `out/`.
Each step states how many units it ran; a step under its minimum fails
even when its command exited 0.

## 4. The hosted half (x86-64)

`run` prints the push for the hosted steps; a human does it:

```sh
git push origin HEAD:refs/heads/sweep/rc-X.Y.Z    # starts sweep.yml
gh run list --workflow sweep.yml --branch sweep/rc-X.Y.Z   # wait for it (several hours)
gh run download <run id> -D <dir> -p 'libcxx-*' -p 'gxx-dg-*' -p 'oss-*'
cargo xtask release-gate import-x86 <dir>
git push origin --delete sweep/rc-X.Y.Z
```

A `sweep/rc-*` push runs the full libc++ suite in six shards, the four
g++.dg shards and their merge, and the OSS matrix (the `sweep = true`
rows with the clang 19 control). Every result artifact carries a
`commit` file; `import-x86` refuses artifacts of another commit and
reads the results files, never the job logs. The boost rows stay
local-only: their kiln wall time on the development host (1.8 to 5.1
hours) with the clang control and a full Boost checkout is too close to
the runners' 355-minute job limit.

## 5. Externals

A check outside the repository is a step like the others:
`--external <id>=<command>` (remembered in the state directory). Its exit
status is the verdict, its wall time is recorded, and the last line it
prints is its result, with `units=<n>`. A release gate requires the
external `downstream`, the downstream reflection-heavy build of the
record's "Downstream build" section.

## 6. Reading a failure

A failed step's record names what fell below its rule; its log is in
`logs/<id>.log`. On a loaded shared host a failure can be the host's: a
test racing on the network filesystem, a timeout under load. The control
decides: run the unit with this RC and with the previous release on the
same host, several times:

```sh
cargo xtask release-gate control oss-glaze glaze --previous <previous kiln++> --times 3
cargo xtask release-gate control libcxx-aarch64 std/thread/... --previous <previous kiln++>
```

`control` prints the table and decides nothing. A human decides: when
both binaries behave the same, it is a flake (write the control's table
in the record, outside the generated regions, as 1.2.1's glaze paragraph
does) and the step is re-run (`run --redo --step <id>`); when only the
RC fails, it is a regression: fix it and the RC moves. A g++.dg case
from pass to a failing class either is a regression or, when its cause
is understood and accepted, goes into
[status/gxx-dg-explained.txt](status/gxx-dg-explained.txt) with the
reason; the record prints that list.

## 7. The record

```sh
cargo xtask release-gate record              # docs/status/release-X.Y.Z.md
cargo xtask release-gate check docs/status/release-X.Y.Z.md
```

`record` fails, listing them, while any step lacks a completed record
for this commit. It writes the tables between `release-gate` markers and
keeps the hand-written text around them (the introduction, the flake
explanations). `check` verifies a committed record: every step present,
one commit and version throughout, every verdict a pass, no generated
region edited by hand. The record goes in through a docs-only pull
request; `record --refresh-baselines` in the same pull request makes this
gate the next one's baseline (the stored counts, the g++.dg baseline,
an emptied explained list).

## 8. The tag and the announcement

The tag `vX.Y.Z` goes on the commit that adds the record, which changes
only docs. `release.yml` publishes the archives from the tag. The
announcement names the version, the CHANGELOG section and the record.

## Patch releases

A patch release (`X.Y.Z`, `Z > 0`) is cut from `release/X.Y`, as
[VALIDATION.md](VALIDATION.md) "Patch releases" describes: the same
gate runs on the branch head (steps 3 to 7), the hosted half from a
`sweep/rc-X.Y.Z` push of that head, and the branch is merged forward
into `main` after the tag.
