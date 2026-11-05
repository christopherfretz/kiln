// [class.union] (C++11, N2544): unions with members that have non-trivial
// special member functions. WP-646 feature probe.
//@ expect: run
//@ std: c++11
//@ strict-diags
//@ stdout: C3 D3 C9 D9 1
extern "C" int printf(const char*, ...);
void* operator new(decltype(sizeof 0), void* p) noexcept { return p; }
struct N {
  int v;
  N(int x) : v(x) { printf("C%d ", v); }
  ~N() { printf("D%d ", v); }
};
union U {
  N n;
  float f;
  U() : f(0) {}
  ~U() {}
};
struct Tagged {
  bool is_n;
  union { N n; int i; };
  Tagged(int x) : is_n(true), n(x) {}
  ~Tagged() { if (is_n) n.~N(); }
};
static_assert(!__is_trivially_destructible(N), "member non-trivial");
int main() {
  U u;
  new (&u.n) N(3);
  u.n.~N();
  { Tagged t(9); }
  printf("%d\n", (int)!__is_trivially_constructible(U));
}
