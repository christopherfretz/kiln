//@ expect: ast
//@ skip(clang): clang 19 does not implement P3394 annotations
//@ oracle: none
// [dcl.attr.annotation]/1: an enumerator and a (non-void) function
// parameter may carry annotations; an enumerator of a member enumeration
// of a class template gets its annotations substituted with it.
enum E { a [[=1]], b [[=2, =3]] = 5, c };
enum class F : unsigned char { x [[=E::b]] };
void params([[=4]] int, [[=5]] int named [[=6]], int last [[=7]] = 0);
struct S {
  S([[=8]] int p);
  void method(int q [[=9]]) const;
};
template <int N> struct T {
  enum M { m [[=N]], n [[=N * 2]] };
};
T<21> t;
