// [temp.arg.nontype]/1, [temp.res.general]/6: a value-dependent non-type
// template argument of a known type in a dependent template-id is checked
// at the definition for a conversion its type cannot make (clang's
// `CheckConvertedConstantExpression` on a dependent argument): an integral
// value to a pointer is not allowed in a converted constant expression, a
// type with no implicit conversion is not convertible. Narrowing waits for
// the value; a parameter of class or dependent type waits for the type.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 44:15 error "conversion from 'const int' to 'int *' is not allowed in a converted constant expression"
//@ diag: 45:15 error "conversion from 'const bool' to 'int *' is not allowed in a converted constant expression"
//@ diag: 46:14 error "conversion from 'const double' to 'int' is not allowed in a converted constant expression"
//@ diag: 47:15 error "conversion from 'const int' to 'double' is not allowed in a converted constant expression"
//@ diag: 48:14 error "value of type 'int *const' is not implicitly convertible to 'int'"
//@ diag: 49:15 error "conversion from 'int *const' to 'bool' is not allowed in a converted constant expression"
//@ diag: 50:15 error "value of type 'const int' is not implicitly convertible to 'int &'"
//@ diag: 51:15 error "value of type 'const E' is not implicitly convertible to 'int *'"
//@ diag: 52:14 error "value of type 'const EC' is not implicitly convertible to 'int'"
//@ diag: 53:15 error "conversion from 'const int' to 'int *' is not allowed in a converted constant expression"
//@ diag: 54:22 error "conversion from 'const int' to 'int *' is not allowed in a converted constant expression"
template<int N> struct I;
template<int *P> struct PP;
template<bool B> struct Bo;
template<double D> struct Db;
enum E { e0 };
enum class EC { x };
struct Lit { int v; constexpr Lit(int x) : v(x) {} };
template<Lit L> struct Cl;
template<int &R> struct Rf;
template<class T, T V> struct TV;
template<int *P> using AP = int;
template<int *P> void fp();
template<int... Ns> struct Pk;

template<class T> struct X {
  static constexpr int i = sizeof(T);
  static constexpr bool b = sizeof(T) > 1;
  static constexpr double d = sizeof(T);
  static constexpr E en = E(sizeof(T));
  static constexpr EC ec = EC(sizeof(T));
  static constexpr int *p = nullptr;
  // Errors at the definition: no conversion is possible whatever T is.
  void f01(PP<i>);
  void f02(PP<b>);
  void f03(I<d>);
  void f04(Db<i>);
  void f05(I<p>);
  void f06(Bo<p>);
  void f07(Rf<i>);
  void f08(PP<en>);
  void f09(I<ec>);
  void f10(AP<i>);
  void f11(TV<int *, i>);
  // Valid, or checked once T is known.
  void g01(Bo<i>);
  void g02(I<en>);
  void g03(Bo<en>);
  void g04(Cl<i>);
  void g05(I<(long long)sizeof(T)>);
  void g06() { fp<i>(); }
  template<class U> void g07(TV<U, i>);
  template<int... Ms> void g08(Pk<i, Ms...>, I<Ms...>);
};
template<class... Ts> void g09(Pk<sizeof(Ts)..., 1>);
