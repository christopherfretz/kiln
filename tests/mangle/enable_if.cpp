// Mangling differential (crates/kiln_codegen/tests/mangle_vs_clang.rs):
// `enable_if` conditions are mangled after the name as
// `Ua9enable_ifI <template-arg>* E` (spec/sema-overload.md §4.3), so
// overloads distinguished only by them get distinct symbols.
enum Mode { A, B };
int f(int n) { return n; }
int f(int n) __attribute__((enable_if(n == 7, ""))) { return 1; }
int f(int n) __attribute__((enable_if(n == 7, ""), enable_if(n > 0, "")))
{ return 2; }
int e(Mode m) __attribute__((enable_if(m == B, "enumerator literal")))
{ return 3; }
int t(int a, int b) __attribute__((enable_if(a < b, "second parameter")))
{ return 4; }
struct S {
  int get(int n) const __attribute__((enable_if(n == 0, ""))) { return 5; }
  static int st(int n) __attribute__((enable_if(n == 2, ""))) { return 6; }
  S(Mode m) __attribute__((enable_if(m == A, ""))) {}
};
template <class T>
T twice(T n) __attribute__((enable_if(n == 0, ""))) { return n; }
template <class T> struct C {
  int h(int n) __attribute__((enable_if(n == 3, ""))) { return 7; }
};
int use() {
  S s(A);
  C<int> c;
  return f(1) + f(7) + e(B) + t(1, 2) + s.get(0) + S::st(2) + twice(0) +
         twice(0L) + c.h(3);
}
