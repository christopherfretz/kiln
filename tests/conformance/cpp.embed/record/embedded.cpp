// [cpp.embed]: an `#embed` is an inclusion row of kind `embed` with
// outcome `embedded`, its name range, its written spelling and the
// resource read (`file`); found relative to the includer, it has no
// search index (spec/pp.md "Preprocessing record", inclusions).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-c23-extensions
//@ pp-record: embedded.record
const char two[] = {
#embed "Inputs/two.bin"
};
static_assert(sizeof two == 2);
