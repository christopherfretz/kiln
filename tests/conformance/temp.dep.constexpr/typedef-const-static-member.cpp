// [temp.dep.constexpr]/2.2: a potentially-constant variable initialized
// with a value-dependent expression makes a reference to it value-dependent,
// also when its `const` comes from a typedef (bsl's
// `typedef const bool Cbool; static Cbool k_INTEGRAL_TYPE = ...;`), so the
// in-class initializer `!a` is checked at instantiation.
//@ strict-diags
template <class T> struct IsI { static const bool k = sizeof(T) > 1; };
struct U { template <class T, int A> struct M; };
template <class T, int A> struct U::M {
  typedef const bool Cbool;
  static Cbool a = IsI<T>::k;
  static Cbool b = !a;
};
template <class T> struct N {
  typedef const bool Cbool;
  static Cbool a = IsI<T>::k;
  static Cbool b = !a;
};
static_assert(U::M<int, 0>::b == false);
static_assert(N<char>::b == true);
