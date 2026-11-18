// [dcl.init]/17.6.3 and [class.copy.elision]/1 in C++14 (C++98/03
// [dcl.init]/14): a copy-initialization by user-defined conversion makes
// a temporary of the destination type and then direct-initializes the
// object from it, a copy that may be elided; kiln checked that copy but
// did not build it, so -fno-elide-constructors did not make it (a
// converting constructor or a conversion function, as a variable, an
// argument or a return value). Found running GCC's C++ testsuite;
// the counts match clang 22. C++17 makes no such temporary.
//@ expect: run
//@ std: c++14
//@ strict-diags
//@ stdout: T x = 1; 0 0 0
//@ stdout: const T x = 1; 0 0 0
//@ stdout: T x = ToT(); 0 0 0
//@ stdout: T x = ToD(); 0 1 0
//@ stdout: byval(1); 0 0 0
//@ stdout: byval(ToT()); 0 0 0
//@ stdout: return 3; 0 0 0
//@ stdout: return ToT(); 0 0 0
//@ stdout: T x(1); 0 0 0
extern "C" int printf(const char *, ...);
int copies, moves, live;
struct T {
  int v;
  T(int x) : v(x) { ++live; }
  T(const T &o) : v(o.v) { ++copies; ++live; }
#if __cplusplus >= 201103L
  T(T &&o) : v(o.v) { ++moves; ++live; }
#endif
  ~T() { --live; }
};
struct D : T {
  D(int x) : T(x) {}
};
struct ToT {
  operator T() const { return T(1); }
};
struct ToD {
  operator D() const { return D(2); }
};
void byval(T) {}
T ret_int() { return 3; }
T ret_conv() { return ToT(); }
void report(const char *what) {
  printf("%s %d %d %d\n", what, copies, moves, live);
  copies = moves = 0;
}
int main() {
  { T x = 1; } report("T x = 1;");
  { const T x = 1; } report("const T x = 1;");
  { T x = ToT(); } report("T x = ToT();");
  { T x = ToD(); } report("T x = ToD();");
  byval(1); report("byval(1);");
  byval(ToT()); report("byval(ToT());");
  { T x = ret_int(); } report("return 3;");
  { T x = ret_conv(); } report("return ToT();");
  { T x(1); } report("T x(1);");
}

