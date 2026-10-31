// [temp.over.link]/4-5: a function template redeclared with an
// equivalent `decltype` return type (parameters named differently) is the
// same template, so the call is not ambiguous.
template <class T> auto f(T t) -> decltype(t + 1);
template <class U> auto f(U u) -> decltype(u + 1) { return u + 1; }

template <class T> auto g(T a, T b) -> decltype(a * b);
template <class T> auto g(T x, T y) -> decltype(x * y) { return x * y; }

int r = f(1) + g(2, 3);
