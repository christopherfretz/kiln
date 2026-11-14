// C23 6.7.11p11, 6.3.2.4: `nullptr` initializes and is assigned to an
// `_Atomic(T *)` object as to a `T *` one (simple-assignment
// constraints on the non-atomic type).
//@ args: -std=gnu23
//@ expect: compile-ok
//@ strict-diags
struct s { _Atomic(int *) p; _Atomic(int) i; };
static struct s v = { nullptr, 1 };
static _Atomic(int *) arr[2] = { nullptr, (void *)0 };
void g(_Atomic(int *) p);
int f(void) {
  struct s a = { .p = nullptr };
  _Atomic(int *) q = nullptr;
  q = nullptr;
  g(nullptr);
  return (a.p == q) + (v.p == arr[0]);
}
