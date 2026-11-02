// [expr.prim.id.dtor]/2, [temp.local]/1: inside a class template, the
// destructor name `~In` (and `In::~In`) names the injected-class-name, the
// dependent `In<T>`, so the destruction of `*this` checks against the
// specialization when instantiated. libstdc++ bits/shared_ptr_base.h:625
// (`this->~_Sp_counted_ptr_inplace()` in `_M_destroy`; every
// std::make_shared); <condition_variable>, <future>
// (docs/status/libstdcxx.md, WP-332).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 0
#include <new>
#include <stdio.h>
int live = 0;
template<class T> struct In {
  T v;
  In() { ++live; }
  ~In() { --live; }
  void d() { this->~In(); }
  void q() { this->In::~In(); }
  void e(In &o) { o.~In(); }
};
int main() {
  alignas(In<long>) unsigned char buf[sizeof(In<long>)];
  auto *a = new (buf) In<int>;
  a->d();
  auto *b = new (buf) In<long>;
  b->q();
  auto *c = new (buf) In<char>;
  c->e(*c);
  printf("%d\n", live);
}
