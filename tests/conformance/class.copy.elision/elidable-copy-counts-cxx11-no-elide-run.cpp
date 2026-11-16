// [class.copy.elision]/1 (C++14 [class.copy]/31) before C++17: the copy
// or move of a temporary into an object of its class (`T x = T();`, a
// function result, a thrown object, a by-value argument, a conditional
// expression's result, an array element) and of a returned local is a
// copy that may be elided; every object made is destroyed (the last
// count). Sema builds it as an elidable constructor call
// (spec/sema-class.md §5.5); under -fno-elide-constructors every copy
// of a temporary is made. Found running GCC's C++ testsuite. Matches
// clang 22.
//@ expect: run
//@ std: c++11
//@ args: -fno-elide-constructors
//@ strict-diags
//@ stdout: T x = T(); 0 1 0
//@ stdout: T x = f(); 0 2 0
//@ stdout: return local; 0 2 0
//@ stdout: throw T(); 0 1 0
//@ stdout: by-value parameter from a temporary 0 1 0
//@ stdout: conditional temporary bound 0 1 0
//@ stdout: discarded conditional 0 1 0
//@ stdout: T x = c ? T() : f(); 0 3 0
//@ stdout: array elements from temporaries 0 2 0
//@ stdout: T x = T(T()); 0 2 0
//@ stdout: T x((T())); 0 1 0
extern "C" int printf(const char *, ...);
int copies, moves, live;
struct T {
  int v;
  T() : v(0) { ++live; }
  T(const T &o) : v(o.v) { ++copies; ++live; }
#if __cplusplus >= 201103L
  T(T &&o) : v(o.v) { ++moves; ++live; }
#endif
  ~T() { --live; }
};
T f() { return T(); }
T g() { T local; return local; }
void byval(T) {}
void cref(const T &) {}
void report(const char *what) {
  printf("%s %d %d %d\n", what, copies, moves, live);
  copies = moves = 0;
}
int main(int argc, char **) {
  { T x = T(); } report("T x = T();");
  { T x = f(); } report("T x = f();");
  { T x = g(); } report("return local;");
  try { throw T(); } catch (T &) {} report("throw T();");
  byval(T()); report("by-value parameter from a temporary");
  { const T &r = argc > 5 ? T() : T(); cref(r); } report("conditional temporary bound");
  argc > 5 ? T() : T(); report("discarded conditional");
  { T x = argc > 5 ? T() : f(); } report("T x = c ? T() : f();");
  { T a[2] = {T(), T()}; } report("array elements from temporaries");
  { T x = T(T()); } report("T x = T(T());");
  { T x((T())); } report("T x((T()));");
}
