// Generic lambdas in templates, carried by the declaration image
// (image-generic-lambda-templ-run.cpp, WP-3410).
export module imggl;
template <class T> concept integral = __is_integral(T);
export namespace gl {
// In a function template's body: the call operator template is one
// level deeper than the function template's head.
template <class T> auto id(T t) { return [](auto x) { return x; }(t); }
template <class T> auto both(T t) {
  auto l = [t](auto x, auto y) { return t + x + y; };
  return l(1, 2) + int(l(1.5, 2.5)) + int(l('\1', 1L));
}
template <class T> auto ret(T t) { return [t](auto x) { return t * x; }; }
// In a class template's member function and member function template.
template <class T> struct C {
  T v;
  auto m() const { return [this](auto x) { return v + x; }(1); }
  template <class U> auto mt(U u) const { return [u, this](auto x) { return v + u + x; }(2); }
};
// Nested generic lambdas.
template <class T> auto nest(T t) {
  return [t](auto x) { return [t, x](auto y) { return t + x + y; }; };
}
// An explicit template parameter list; a constrained placeholder.
template <class T> auto expl(T t) { return []<class U>(U u) { return int(sizeof(U)) + u; }(t); }
template <class T> auto cons(T t) { return [](integral auto x) { return x * 2; }(t); }
// In a default argument and a variable template's initializer: the
// closure's context is the namespace, its owner the template's pattern.
template <class T> int dflt(T t, int (*p)(int) = [](auto x) { return x + 1; }) { return p(int(t)); }
template <class T> inline auto vt = [](auto x) { return T(x) + 1; };
// The conversion to a pointer to function; constant evaluation.
template <class T> int conv(T t) { int (*p)(T) = [](auto x) { return int(x) + 3; }; return p(t); }
template <class T> constexpr int ce(T t) { return [](auto x) { return x * 3; }(t); }
} // namespace gl
