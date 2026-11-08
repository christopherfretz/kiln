// A call of a `[[noreturn]]` virtual function through the vtable returns:
// overriders need not inherit the attribute, so the call site has no
// `noreturn` and no called function carries it (clang `CallBase::
// doesNotReturn`), and the code after it is kept. A qualified call of the
// same function does not return. kiln ended the block after both.
//@ expect: ir
//@ strict-diags
struct S { [[noreturn]] virtual void v(); };
int f(int x, S *s) {
  if (x) s->v();
  if (x > 1) s->S::v();
  return x;
}
