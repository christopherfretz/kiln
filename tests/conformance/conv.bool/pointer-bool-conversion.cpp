// [conv.bool]/1, [expr.unary.op]/3, [stmt.pre]/4: a pointer that cannot be null
// converted to bool (clang DiagnoseAlwaysNonNullPointer, -Wpointer-bool-conversion,
// -Wundefined-bool-conversion) at every contextual and implicit conversion: if,
// while, for, do, ?:, !, &&, ||, direct initialization, return, an argument;
// the address/array/function forms with the '&' and '()' notes, 'this', a
// reference, a function template-id; again in every instantiation. Not for an
// explicit cast, a macro body, an unevaluated operand (WP-421).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 56:21 warning "address of 'x' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 57:41 warning "address of 'x' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 64:30 warning "address of 'this->v' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 64:21 warning "'this' pointer cannot be null in well-defined C++ code; pointer may be assumed to always convert to true [-Wundefined-bool-conversion]"
//@ diag: 67:8 warning "address of 'x' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 68:10 warning "address of array 'arr' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 69:10 warning "address of function 'fn' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 69:10 note "prefix with the address-of operator to silence this warning"
//@ diag: 70:17 warning "address of 'x' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 71:12 warning "address of 'x' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 72:13 warning "address of function 'fn' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 72:13 note "prefix with the address-of operator to silence this warning"
//@ diag: 73:13 warning "address of 'x' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 74:17 warning "address of array 'arr' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 76:11 warning "address of 'x' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 78:9 warning "address of 'x' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 79:7 warning "address of function 'fb' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 79:7 note "prefix with the address-of operator to silence this warning"
//@ diag: 79:7 note "suffix with parentheses to turn this into a function call"
//@ diag: 80:7 warning "address of function 'fd' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 80:7 note "prefix with the address-of operator to silence this warning"
//@ diag: 80:7 note "suffix with parentheses to turn this into a function call"
//@ diag: 81:7 warning "address of function 'ft<int>' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 81:7 note "prefix with the address-of operator to silence this warning"
//@ diag: 82:8 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; pointer may be assumed to always convert to true [-Wundefined-bool-conversion]"
//@ diag: 83:8 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; pointer may be assumed to always convert to true [-Wundefined-bool-conversion]"
//@ diag: 51:6 note "'ri' returns a reference"
//@ diag: 91:14 warning "comparison of address of 'x' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
//@ diag: 57:41 warning "address of 'x' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 95:3 note "in instantiation of function template specialization 'tf<int>' requested here"
//@ diag: 57:41 warning "address of 'x' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 96:3 note "in instantiation of function template specialization 'tf<double>' requested here"
//@ diag: 60:22 warning "address of 'this->m' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 97:13 note "in instantiation of member function 'TS<int>::g' requested here"
//@ diag: 60:22 warning "address of 'this->m' will always evaluate to 'true' [-Wpointer-bool-conversion]"
//@ diag: 98:14 note "in instantiation of member function 'TS<long>::g' requested here"
int x;
int arr[2];
void fn();
bool fb();
bool fd(int = 0);
int &ri();
void take(bool);
template <class T> T *ft();
#define ADDR &x
#define CHECK(e) if (e) {}
bool r1() { return &x; }
template <class T> bool tf(T) { return &x; }
template <class T> struct TS {
  int m;
  bool g() { return &m; }
};
struct C {
  int v;
  bool h() { return this && &v; }
};
void contexts(bool b, int &r) {
  if (&x) {}
  while (arr) {}
  for (; fn;) {}
  do {} while (&x);
  int t = &x ? 1 : 2;
  bool n = !fn;
  bool a = &x && b;
  bool o = b || arr;
  bool s = static_cast<bool>(&x);
  bool e(&x);
  bool c = (bool)&x;
  take(&x);
  if (fb) {}
  if (fd) {}
  if (ft<int>) {}
  if (&r) {}
  if (&ri()) {}
  (void)t, (void)n, (void)a, (void)o, (void)s, (void)e, (void)c;
}
void silent() {
  if (ADDR) {}
  CHECK(&x)
  bool u = sizeof(!&x);
  decltype(!&x) v = true;
  bool p = (&x) != 0;
  (void)u, (void)v, (void)p;
}
int main() {
  tf(1);
  tf(2.0);
  TS<int>().g();
  TS<long>().g();
}
