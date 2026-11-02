// [dcl.fct]/22: an `auto` in the trailing return type of a parameter's
// function declarator invents a template parameter (constrained: `Tk`);
// the decl-specifier `auto` before it does not (`n` stays a function).
template <class T> concept C = true;
int fi(int);
void n(auto (*cb)(int) -> int) {}
void t1(auto (*cb)(int) -> auto) {}
void t2(auto (*cb)(int) -> C auto) {}
void t3(int x, auto (&cb)(int) -> auto, auto y) {}
template <class T> void t4(T a, auto (*p)(T b) -> decltype(a + b)) {}
void use() {
  n(fi);
  t1(fi);
  t2(fi);
  t3(1, fi, 2L);
  t4(1, fi);
  auto l = [](auto (*cb)(int) -> auto) {};
  l(fi);
}
