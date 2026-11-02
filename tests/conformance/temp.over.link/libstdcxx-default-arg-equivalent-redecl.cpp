// [temp.over.link]/5-7, [temp.arg.general]/9: a dependent template-id that
// leaves a parameter to its default argument (`vec<T>`) is the same type as
// one writing the default out (`vec<T, T*>`), so the second declaration
// redeclares the first and the call is not ambiguous (the root of the
// libstdc++ `swap` ambiguity: `enable_if<B>` vs `enable_if_t<B>`, WP-323).
// Verified against clang 19.
//@ expect: compile-ok
template<class T, class A = T *> struct vec {};
template<class T> int f(vec<T>);
template<class T> int f(vec<T, T *>) { return 1; }
template<bool B, class T = void> struct enable_if {};
template<class T> struct enable_if<true, T> { using type = T; };
template<class T> typename enable_if<sizeof(T) == 4>::type g(T);
template<class T> typename enable_if<sizeof(T) == 4, void>::type g(T) {}
int h() { g(1); return f(vec<int>()); }
