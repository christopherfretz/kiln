//@ expect: ast
//@ oracle: clang
//@ requires: clang>=22
// [temp.deduct.guide]: user-declared deduction guides dump as clang's
// CXXDeductionGuideDecl (a template one under its FunctionTemplateDecl);
// implicit guides are not printed.
template<class T> struct S { S(T); };
template<class T> S(T *) -> S<T>;
S(int x) -> S<long>;
explicit S(char) -> S<short>;
S s(1);
namespace N {
template<class T, class U> struct P { P(T, U); };
template<class T> P(T *, T) -> P<T, int>;
P(double, double) -> P<float, float>;
}
N::P p(1.0, 2.0);
struct O {
  template<class T> struct I { I(T); };
  I(bool) -> I<int>;
};
O::I i(true);
