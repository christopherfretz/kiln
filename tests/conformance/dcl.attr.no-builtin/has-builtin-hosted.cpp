// [cpp.cond]/5 `-fbuiltin` cancels `-fno-builtin` and `-fhosted`
// cancels `-ffreestanding`: `__has_builtin(memcpy)` is 1 again. A
// `-fbuiltin` alone does not cancel `-ffreestanding`. spec/pp.md
// "feature queries", WP-808.
//@ expect: compile-ok
//@ strict-diags
//@ args: -fno-builtin -ffreestanding -fbuiltin -fhosted
static_assert(__has_builtin(memcpy));
static_assert(__STDC_HOSTED__ == 1);
