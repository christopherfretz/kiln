// [cpp.cond]/5, [cpp.embed]: a `__has_embed` query is a `query` row with
// its operand as written and its value (1: found and not empty, 2: found
// and empty, 0: not found); the condition reads it as an `expanded` read
// of the builtin (spec/pp.md "Preprocessing record", queries).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-c23-extensions --embed-dir=%{test_dir}/Inputs/dirs/a
//@ pp-record: has-embed.record
#if __has_embed("Inputs/two.bin")
int found;
#endif
#if __has_embed(<empty.bin>) == 2
int empty;
#endif
#if !__has_embed("Inputs/missing.bin" limit(1))
int missing;
#endif
