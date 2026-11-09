// [dcl.fct.def.general]/2 valid twin: block-scope function declarations,
// a local class's member function definitions, a lambda (generic or with
// a template head) and a function-try-block's function at namespace
// scope all compile (WP-1091).
//@ expect: compile-ok
int g(int);
template<class T> int t() try { return 0; } catch (...) { return 1; }
int f() {
  int h(int);
  struct L { int m() { return 1; } int n(int x) { return x; } };
  auto a = []<class T>(T x) { return x; };
  auto b = [](auto x) { return x; };
  return L{}.m() + L{}.n(1) + a(1) + b(1) + t<int>();
}
int g(int x) { return x; }
