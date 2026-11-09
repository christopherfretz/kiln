// [basic.def.odr]/3,5, [expr.prim.lambda.capture]/8 (WP-1144): a by-copy
// default capture captures a local only if the lambda odr-uses it. The
// potential results of `e.m` (a non-static data member), of a subscript
// with an array operand `e[i]`/`i[e]`, of `e.*pm`, of `(e)`, of both arms
// of `b ? e1 : e2`, of the right operand of a comma and of `__extension__
// e` include those of `e`; naming a constexpr object there, under an
// lvalue-to-rvalue conversion of a non-volatile, non-class operand, is no
// odr-use, so it is not captured. Odr-uses are captured: `&ca.v`, binding
// a reference, a variable with a mutable member, a volatile glvalue read,
// a discarded `(void)ca` (clang never applies the discarded-value bullet), a
// class copy, `->`, the object of a static data member access, and a
// generic lambda's `return ca.v` (no conversion until instantiation).
// clang 19 and 22 agree on every size; the program prints the sizes and
// the values, compared with the oracle (closure layout and code).
//@ expect: run
//@ strict-diags
extern "C" int printf(const char *, ...);
struct A {
  int v;
  int arr[2];
};
struct M {
  int v;
  mutable int m;
};
struct B {
  A a;
};
struct S {
  static constexpr int sm = 3;
  int v;
};
struct V {
  int v;
};

template <class T> int in_template() {
  constexpr T ct{4, {5, 6}};
  auto l = [=] { return ct.v + ct.arr[1]; };
  static_assert(sizeof(l) == 1);
  return l();
}

#define SHOW(l, ...) printf("%-3s %2zu %d\n", #l, sizeof(l), l(__VA_ARGS__))

int main() {
  constexpr A ca{4, {5, 6}};
  constexpr int carr[2] = {1, 2};
  constexpr B cb{{7, {8, 9}}};
  constexpr M cm{1, 2};
  constexpr M cms[2] = {{1, 2}, {3, 4}};
  constexpr int A::*pm = &A::v;
  constexpr A cas[2] = {{1, {2, 3}}, {4, {5, 6}}};
  constexpr S cs{1};
  constexpr V cv{2};

  // Not odr-uses: nothing is captured.
  auto n1 = [=] { return ca.v; };
  auto n2 = [=] { return carr[1]; };
  auto n3 = [=] { return cb.a.arr[1]; };
  auto n4 = [=] { return ca.*pm; };
  auto n5 = [=](bool b) { return (b ? ca : ca).v; };
  auto n6 = [=] { return ((void)0, ca).v; };
  auto n7 = [=] { return cas[1].arr[0]; };
  auto n8 = [=] { return (ca).v; };
  auto n9 = [=] { return ca.arr[1]; };
  auto n10 = [=] { return 1[ca.arr]; };
  auto n11 = [=] { return __extension__ ca.v; };
  auto n12 = [=](auto) { return ca.v + 0; };
  auto n13 = [=](auto) -> int { return ca.v; };
  // clang looks only at the variable's own type for mutable members, not
  // at an array's elements.
  auto n14 = [=] { return cms[0].v; };
  static_assert(sizeof(n1) == 1 && sizeof(n2) == 1 && sizeof(n3) == 1);
  static_assert(sizeof(n4) == 1 && sizeof(n5) == 1 && sizeof(n6) == 1);
  static_assert(sizeof(n7) == 1 && sizeof(n8) == 1 && sizeof(n9) == 1);
  static_assert(sizeof(n10) == 1 && sizeof(n11) == 1 && sizeof(n12) == 1);
  static_assert(sizeof(n13) == 1 && sizeof(n14) == 1);

  // Odr-uses: the variable is captured.
  auto c1 = [=] { return cm.v; };
  auto c2 = [=] { return *&ca.v; };
  auto c3 = [=] {
    const int &r = ca.v;
    return r;
  };
  auto c4 = [=] {
    (void)ca;
    return 0;
  };
  auto c5 = [=] {
    A x = ca;
    return x.v;
  };
  auto c6 = [=] { return (&ca)->v; };
  auto c7 = [=] { return cs.sm; };
  auto c8 = [=] { return static_cast<const volatile V &>(cv).v; };
  auto c9 = [=](auto) { return ca.v; };
  auto c10 = [=](auto) { return carr[1]; };
  static_assert(sizeof(c1) == sizeof(M) && sizeof(c2) == sizeof(A));
  static_assert(sizeof(c3) == sizeof(A) && sizeof(c4) == sizeof(A));
  static_assert(sizeof(c5) == sizeof(A) && sizeof(c6) == sizeof(A));
  static_assert(sizeof(c7) == sizeof(S) && sizeof(c8) == sizeof(V));
  static_assert(sizeof(c9) == sizeof(A) && sizeof(c10) == sizeof(carr));

  SHOW(n1);
  SHOW(n2);
  SHOW(n3);
  SHOW(n4);
  SHOW(n5, true);
  SHOW(n6);
  SHOW(n7);
  SHOW(n8);
  SHOW(n9);
  SHOW(n10);
  SHOW(n11);
  SHOW(n12, 0);
  SHOW(n13, 0);
  SHOW(n14);
  SHOW(c1);
  SHOW(c2);
  SHOW(c3);
  SHOW(c4);
  SHOW(c5);
  SHOW(c6);
  SHOW(c7);
  SHOW(c8);
  SHOW(c9, 0);
  SHOW(c10, 0);
  printf("template %d\n", in_template<A>());
}
