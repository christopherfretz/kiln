// [cpp.embed]/3: a resource found in the second `--embed-dir` directory
// is an `embedded` row with the file read and the search index 1 (`dir`);
// the first directory's empty resource has index 0 (spec/pp.md
// "Preprocessing record", inclusions).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-c23-extensions --embed-dir=%{test_dir}/Inputs/dirs/a --embed-dir=%{test_dir}/Inputs/dirs/b
//@ pp-record: embed-dir.record
const char three[] = {
#embed <three.bin>
};
static_assert(sizeof three == 3);
const char none[] = {
#embed <empty.bin> if_empty(0)
};
static_assert(sizeof none == 1);
