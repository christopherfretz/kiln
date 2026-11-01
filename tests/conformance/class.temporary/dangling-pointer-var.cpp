// [class.temporary]/4, -Wdangling: a pointer variable initialized with
// the address of a temporary (an array prvalue decayed, directly, through
// a member or a conditional arm, or by `p{...}`) dangles at the end of
// the full-expression; a pointer to a lifetime-extended array or a named
// array does not (WP-227).
//@ strict-diags
//@ diag: 20:16 warning "temporary whose address is used as value of local variable 'p' will be destroyed at the end of the full-expression"
//@ diag: 26:18 warning "temporary whose address is used as value of local variable 'q' will be destroyed at the end of the full-expression"
//@ diag: 27:18 warning "temporary whose address is used as value of local variable 's' will be destroyed at the end of the full-expression"
//@ diag: 28:25 warning "temporary whose address is used as value of local variable 't' will be destroyed at the end of the full-expression"
//@ diag: 28:32 warning "temporary whose address is used as value of local variable 't' will be destroyed at the end of the full-expression"
//@ diag: 29:16 warning "temporary whose address is used as value of local variable 'w' will be destroyed at the end of the full-expression"
//@ diag: 30:16 warning "temporary whose address is used as value of local variable 'x' will be destroyed at the end of the full-expression"
//@ diag: 31:18 warning "temporary whose address is used as value of local variable 'c' will be destroyed at the end of the full-expression"
//@ diag: 32:12 warning "temporary whose address is used as value of local variable 'a' will be destroyed at the end of the full-expression"
//@ diag: 38:44 warning "temporary whose address is used as value of local variable 'p' will be destroyed at the end of the full-expression"
//@ diag: 39:15 note "in instantiation of function template specialization 'h<int>' requested here"
using U = int[2];
struct S { int a[2]; int* q; };
const int* p = U{1, 2};
int g[2];
const int (&r)[2] = U{1};
const int* p1 = r;
const int* p2 = S{}.q;
void f() {
  const int* q = U{1, 2};
  const int* s = S{}.a;
  const int* t = true ? U{1} : U{2};
  const int* w(U{1});
  const int* x{U{1}};
  const int* c = (int[]){1, 2};
  auto a = U{1};
  int b[2];
  int* ok = b;
  const int* ok2 = true ? b : g;
  (void)q, (void)s, (void)t, (void)w, (void)x, (void)c, (void)a, (void)ok, (void)ok2;
}
template <class T> void h() { const T* p = U{1}; (void)p; }
template void h<int>();
