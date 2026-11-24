// Function templates whose deduction fails silently in an importer
// (subst-failed-module-image.cpp).
export module subst_failed_mod;
export template <class T> auto sf_g(T t) -> decltype(t.size());
export int sf_g(...);
export template <class T, class U> struct sf_X {};
export template <class T> struct sf_X<T, int> { using t = int; };
export template <class U> struct sf_X<int, U> { using t = int; };
export template <class T> auto sf_k(T) -> typename sf_X<T, int>::t;
export long sf_k(...);
