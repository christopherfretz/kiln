// [dcl.attr.grammar]/5 clang folds a `pass_object_size` argument (and
// `__builtin_object_size`) with its lenient lvalue designator
// (`ExprConstant` `tryEvaluateBuiltinObjectSize`, `determineEndOffset`):
// an index outside the array or before its start marks the designator
// invalid but keeps the byte offset, so the size is 0; a reinterpreting
// cast (`(char *)&s`) keeps the offset, so `+ 4` is the object's size
// less 4; a member of a pointer with no known value keeps its designator
// (an invalid base), so type 1/3 of `ps->a` is the member's size. The
// Oven fails these and kiln passes `llvm.objectsize`, which only weakens
// a check at -O0. spec/codegen.md "pass_object_size", WP-808.
//@ xfail(kiln): WP-808 follow-up, the Oven has no lenient designator for object-size folds (out-of-bounds offsets, reinterpreted pointers, invalid bases)
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -ftrivial-auto-var-init=uninitialized
#define PS(t) __attribute__((pass_object_size(t)))
void f0(const void *PS(0) p);
void f1(const void *PS(1) p);
void f3(const void *PS(3) p);
struct S { char a[10]; int arr[4]; double d; };
char gbuf[32];
void lenient(S *ps) {
  S s;
  f0(&gbuf[40]); f0(gbuf - 1);
  f0((char *)&s + 4); f1((char *)&s + 4);
  f1(ps->a); f3(ps->a);
}
