// Specializations of function templates that are not inline whose local
// classes and closures name TU-local entities, used where that is no
// exposure (exposure-local-class-instantiation-importer-run.cpp; the
// module-only checks are basic.link/exposure-local-class-instantiation.cpp).
export module xlci;
static int helper(int x) { return x + 1; }
namespace { struct S { int v; }; }

template <class T> auto returned(T x) { return [=] { return helper(x); }; }
template <class T> auto not_returned(T x) { struct L { constexpr int g(T y) { return helper(y); } }; return L().g(x); }
template <class T> auto not_inline(T) { struct L { int g(T y) { return helper(y); } }; return L(); }
template <class T> auto field(T x) { struct L { S s; }; return L{{int(x)}}; }

export int ignored(int x) { return returned(x)() + field(x).s.v; }
export inline int inline_not_returned(int x) { return not_returned(x); }
export inline int inline_not_inline(int x) { return not_inline(x).g(x); }
