// [temp.deduct.type]/10: when P and A are function types from taking the
// address of a function template ([temp.deduct.funcaddr]) or from a
// declaration ([temp.deduct.decl]), a top-level parameter `T&&` that is a
// forwarding reference paired with an lvalue reference `U&` is adjusted
// to `T`, so `T` is deduced as `U&`. Packs `Args&&...` too
// (libcxx/utilities/function.objects/func.require/bullet_7.pass.cpp).
//@ expect: run
template <class T> int h(T&&) { return 1; }
template <> int h(int&) { return 2; }  // T = int&

template <class... Args> int f(Args&&...) { return 3; }
template <class... Args> int f(Args&&..., ...) { return 4; }

template <class Sig, Sig* P> int call(int& x) { return P(x); }

template <class T> int k(T&&, T&&) { return 5; }

// Not at the top level: `void (*)(T&&)` is matched as written.
template <class T> int nested(void (*)(T&&)) { return 6; }
int nested(...) { return 7; }
void takes_lref(int&) {}

int main() {
  int x = 0;
  int (*p)(int&) = h;
  int (*p2)(int&&) = h;
  int (*q)(int&, long&) = f;
  int (*v)(int&, ...) = f;
  int (*r)(int&, int&) = k;
  if (p(x) != 2 || p2(0) != 1 || !q) return 1;
  if (v(x) != 4 || r(x, x) != 5) return 2;
  if (call<int(int&), f>(x) != 3) return 3;
  if (nested(takes_lref) != 7) return 4;
  return 0;
}
