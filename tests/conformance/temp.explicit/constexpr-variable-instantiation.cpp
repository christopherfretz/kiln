// [temp.explicit]/3, [dcl.constexpr]/9: `constexpr` in an explicit
// instantiation of a variable template makes the declared type const, so it
// matches a `constexpr` variable template's type.
//@ expect: compile-ok
template <class T> constexpr int v = sizeof(T);
template constexpr int v<short>;
template const int v<long>;
extern template constexpr int v<char>;
static_assert(v<short> == sizeof(short));
