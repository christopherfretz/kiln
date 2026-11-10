// [class.temporary]/6, CWG1815: a temporary bound to a reference member by
// a default member initializer that a variable's aggregate initialization
// uses is lifetime-extended with the variable, in a nested aggregate and
// for a static variable too, and so is an initializer list's backing
// array; the member initializer's other temporaries end with the
// variable's full-expression. Each use gets its own copy of the member's
// initializer with fresh temporaries (spec/sema.md §7.6a). clang 20+ and
// GCC 16.1 agree; clang 19 does not extend (dangling-default-member-init-19.cpp).
// WP-1177.
//@ expect: run
//@ std: c++23
//@ strict-diags
//@ requires: clang>=22
//@ stdout: ~D8 ~D8 | r 7 8 ~D7 | l 2 ~D2 ~D1 | ~D8 n 7 ~D7 | g 7 | ~D7 end
namespace std {
template <class E> class initializer_list {
  const E* b;
  decltype(sizeof 0) n;
  constexpr initializer_list(const E* b, decltype(sizeof 0) n) : b(b), n(n) {}
public:
  constexpr initializer_list() : b(nullptr), n(0) {}
  constexpr decltype(sizeof 0) size() const { return n; }
};
} // namespace std
extern "C" int printf(const char*, ...);
struct D {
  int v;
  ~D() { printf("~D%d ", v); }
};
struct R {
  const D& r = D{7};
  int y = D{8}.v;
};
struct L {
  std::initializer_list<D> l = {D{1}, D{2}};
};
struct N {
  R a;
};
// Destroyed after `g` and its extended temporary: ends the output line.
struct Nl {
  ~Nl() { printf("end\n"); }
} nl;
R g{};

void f1() {
  R r{};
  printf("| r %d %d ", r.r.v, r.y);
}
void f2() {
  L l{};
  printf("| l %d ", (int)l.l.size());
}
void f3() {
  N n{};
  printf("n %d ", n.a.r.v);
}
int main() {
  f1();
  f2();
  printf("| ");
  f3();
  printf("| g %d | ", g.r.v);
}
