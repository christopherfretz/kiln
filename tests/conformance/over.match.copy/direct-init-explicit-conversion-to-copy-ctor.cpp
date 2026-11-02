// [dcl.init.general]/16.6.2.1, [over.match.copy]/1.2, [over.match.ref]/1.1:
// a direct-initialization of a class object from one expression of an
// unrelated class considers the constructors, and the temporary bound to
// a copy or move constructor's reference parameter may come from an
// explicit conversion function (also a conversion function template)
// yielding (a reference to) cv T. CWG2327 (clang's workaround): a
// conversion function template yielding cv T initializes the object
// directly, so an explicit one wins in a direct-initialization only, and
// a deleted explicit one makes the direct-initialization ill-formed while
// the copy-initialization is fine (libc++ 19 tuple convert_move.pass.cpp,
// `BonkersBananas`).
//@ expect: run
struct R {
  int a;
  R(int x) : a(x) {}
  R(const R& o) : a(o.a + 1) {}
};
R glob(7);
struct G { explicit operator R&() { return glob; } };
struct K { template <class T> explicit operator T&() { return glob; } };
struct E { template <class T> explicit operator T() && { return T(30); } };
struct BB {
  template <class T> operator T() &&;
  template <class T, class = void> explicit operator T() && = delete;
};
struct P { P(int, int); P(const P&); P(P&&); };
static_assert(__is_constructible(R, G) && !__is_convertible(G, R));
static_assert(__is_constructible(R, K) && !__is_convertible(K, R));
static_assert(__is_constructible(R, E) && !__is_convertible(E, R));
static_assert(__is_convertible(BB, P) && !__is_constructible(P, BB));
struct M {
  R m;
  M() : m(G{}) {}
};
int main() {
  R r1(G{});
  R r2 = R(K{});
  R r3{G{}};
  R r4 = static_cast<R>(K{});
  R* r5 = new R(G{});
  M m;
  R r6(E{});
  int got = r1.a + r2.a + r3.a + r4.a + r5->a + m.m.a; // 6 copies of glob
  delete r5;
  return got == 6 * 8 && r6.a == 30 ? 0 : 1;
}
