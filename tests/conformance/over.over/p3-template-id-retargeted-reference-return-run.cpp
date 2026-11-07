// [over.over]/1, /3: a function template-id whose explicit arguments
// alone name one specialization is still an overload set when a target
// type is known; a reference to function, a pointer to function in a
// `return`, and a direct-initialization all select the template the
// target deduces, not the one the explicit arguments complete.
//@ expect: run
//@ stdout: 273849
extern "C" int printf(const char*, ...);
template<int N> void wrap(void*) {}
int acc = 0;
template<int N, class T0> void wrap(void*, T0 a0) { acc = acc * 100 + N * 10 + a0; }
using F = void(void*, int);
F* get() { return wrap<3>; }
int main() {
  F& r = wrap<2>;
  r(nullptr, 7);
  get()(nullptr, 8);
  F* p(&wrap<4>);
  p(nullptr, 9);
  printf("%d\n", acc);
}
