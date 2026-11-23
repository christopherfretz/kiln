// [dcl.attr.grammar]/5: `maybe_undef` is not inherited by a later
// declaration's parameter, and a call freezes its argument only when the
// parameter of the declaration it names has one of its own (clang
// `CodeGenFunction::EmitCallArgs`): `m1` and `m2` (outside the block)
// do not, `m2` in the block and `m3` (its latest declaration) do.
// spec/sema.md §4.5 "Parameter attributes", WP-2476.
//@ expect: ir
void m1(int x __attribute__((maybe_undef)));
void m1(int x);
void g1(int a) { m1(a); }

void m2(int x);
void g2(int a) { void m2(int x __attribute__((maybe_undef))); m2(a); }
void g3(int a) { m2(a); }

void m3(int x);
void m3(int x __attribute__((maybe_undef)));
void g4(int a) { m3(a); }
