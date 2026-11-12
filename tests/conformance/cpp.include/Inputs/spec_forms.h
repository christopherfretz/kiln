#ifndef SPEC_FORMS_H
#define SPEC_FORMS_H
namespace sf {
template <class T> concept four = sizeof(T) == 4;
template <class T> concept eight = sizeof(T) == 8;

// Constrained partial specializations.
template <class T> struct A { static constexpr int k = 0; };
template <four T> struct A<T> { static constexpr int k = 1; };
template <class T> requires eight<T> struct A<T *> { static constexpr int k = 2; };
template <class T> struct A<T *> { static constexpr int k = 3; };
inline int a_sum() { return A<char>::k + A<int>::k * 10 + A<double *>::k * 100 + A<char *>::k * 1000; }

// A partial specialization declared before its definition.
template <class I> struct traits;
template <class T> struct traits<T *>;
template <class I> struct traits { using value = int; };
template <class T> struct traits<T *> { using value = T; static constexpr int k = 2; };
using cp = traits<char *>::value;

// A dependent expression as a partial specialization argument.
template <class T, unsigned long = 0> struct has { static constexpr bool value = false; };
template <class T> struct has<T, sizeof(T) * 0> { static constexpr bool value = true; };

// Variable templates with constrained and partial specializations.
template <class T> inline constexpr int k_v = 1;
template <class T> inline constexpr int k_v<T *> = 2;
template <class T> requires eight<T> inline constexpr int k_v<T *> = 3;
template <eight T> inline constexpr int c_v = sizeof(T);
inline constexpr int ks = k_v<char *> * 100 + k_v<long *> * 10 + k_v<int> + c_v<long>;

// Member function template specializations of a class specialization.
template <class R, int P = 1> struct dur {
  R r;
  template <class R2> constexpr explicit dur(const R2 &x) : r(static_cast<R>(x) * P) {}
  template <class R2, int Q> constexpr dur(const dur<R2, Q> &d) : r(d.r * Q / P) {}
  constexpr R count() const { return r; }
};
using hours = dur<long, 60>;
inline long in_mins(hours h) { return dur<long>(h).count(); }

// Value and pack explicit template arguments in a body.
template <char... C> struct digits { static constexpr long v = 0; };
template <char C, char... R> struct digits<C, R...> {
  static constexpr long p() { long x = 1; for (unsigned i = 0; i < sizeof...(R); ++i) x *= 10; return x; }
  static constexpr long v = (C - '0') * p() + digits<R...>::v;
};
struct sec { long n; };
template <char... C> constexpr sec operator""_s() { return sec{digits<C...>::v}; }
inline long secs() { return (4321_s).n; }

// A member function with a trailing requires-clause.
template <class T> struct W { int f() const requires four<T> { return 1; } int g() const { return 2; } };
inline int w_use() { return W<int>{}.f() + W<char>{}.g(); }
// A constrained member template of a completed specialization.
template <class T> struct CM {
  template <four U> CM(U, int) : k(2) {}
  CM() : k(0) {}
  int k;
};
inline int cm_use() { CM<char> c; return c.k; }
} // namespace sf
#endif
