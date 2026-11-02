// [dcl.fct]/2, /22: the decl-specifier `auto` of a parameter declarator with
// a trailing return type is that function type's `auto`, not a placeholder:
// `void f(auto (*cb)(int) -> int)` is an ordinary function. An `auto` in such
// a trailing return type of the function's own clause (or a lambda's) does
// invent a template parameter, constrained or not; a parameter of function
// type (`auto cb(int) -> int`) is adjusted to a pointer. The parameter `a`
// named in a nested prototype's trailing decltype is substituted.
//@ expect: run
//@ stdout: 7 8 9 10 11 12 13 14
extern "C" int printf(const char*, ...);
template <class T> concept C = true;
int inc(int x) { return x + 1; }
int f1(auto (*cb)(int) -> int) { return cb(6); }
int f1(int (*cb)(int));
int f2(auto (*cb)(int) -> auto) { return cb(7); }
int f3(auto (*cb)(int) -> C auto) { return cb(8); }
int f4(auto (&cb)(int) -> int) { return cb(9); }
int f5(auto cb(int) -> int) { return cb(10); }
template <class T> T f6(T a, auto (*p)(T b) -> decltype(a + b)) { return p(a + 1); }
int f7(int x, auto (*cb)(int) -> auto) { return cb(x); }
int main() {
  auto l = [](auto (*cb)(int) -> auto) { return cb(12); };
  printf("%d %d %d %d %d %d %d %d\n", f1(inc), f2(inc), f3(inc), f4(inc), f5(inc),
         f6(10, inc), l(inc), f7(13, inc));
}
