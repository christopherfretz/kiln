# Reporting bugs

Report kiln bugs at https://github.com/christopherfretz/kiln/issues.

## What counts as a bug

All of these are bugs:

- **A crash** (an internal compiler error).
- **A wrong verdict**: a valid program rejected, or an invalid one
  accepted, checked against the standard or against clang.
- **A miscompile**: the program builds but behaves differently than it
  does with clang or GCC.
- **A diagnostic** that is wrong or misleading.
- **A constant-evaluation slowdown or memory blowup** that clang or GCC
  does not show.

## Crashes

When kiln crashes it prints a report like this:

```
kiln++: error: internal compiler error: <message>
kiln++: note: while compiling 'foo.cpp' (phase: parse, at foo.cpp:12:1)
kiln++: note: in compiler source crates/kiln_parse/src/decl.rs:123:9 (thread 'kiln')
kiln 0.0.1 (0123456789ab)
kiln++: note: please submit a bug report to https://github.com/christopherfretz/kiln/issues and include the crash reproducer
kiln++: note: set KILN_BACKTRACE=1 to include the compiler backtrace
kiln++: note: preprocessed source and associated run script are located at:
kiln++: note: diagnostic msg: /tmp/foo-3fa9c1.ii
kiln++: note: diagnostic msg: /tmp/foo-3fa9c1.sh
```

The two files are the crash reproducer:

- `foo-3fa9c1.ii` is your input after preprocessing, so it needs no
  headers.
- `foo-3fa9c1.sh` is the command that crashed, rewritten to compile
  the `.ii`. Run `sh foo-3fa9c1.sh` to check that it still crashes.

Attach both files to the issue, along with the whole report. Look over the
`.ii` first: it contains your source and every header it included. If you
cannot share it, reduce it (below) or describe the crash as best you can.

Controls:

- `-fcrash-diagnostics-dir=<dir>` writes the reproducer to `<dir>`. By
  default it goes to `$TMPDIR`, or `/tmp` if that is unset.
- `-fno-crash-diagnostics` writes no reproducer. Build systems and test
  harnesses that expect crashes use it.
- `KILN_BACKTRACE=1` (or `RUST_BACKTRACE=1`) adds kiln's backtrace to
  the report. Include it if you can.

The exit status after a crash is 1, as with clang. Under
`-fno-crash-diagnostics` it is 70.

## Other bugs

Please include:

- the output of `kiln++ --version`, and the commit if you built kiln
  yourself;
- the exact command line;
- the smallest input that shows the problem: preprocess with `-E` to
  get a single file, then reduce it, for example with
  [C-Reduce](https://github.com/csmith-project/creduce) or
  [cvise](https://github.com/marxin/cvise);
- what kiln did, and what clang 19 (or GCC 16.1, for reflection and
  contracts) does with the same input.

For a performance problem, also give the wall time and peak memory for
both compilers. `/usr/bin/time -v` reports both, and
`-fkiln-instantiation-report` / `-fkiln-memory-report` show where kiln
spent them.
