// [temp.explicit]/3: an explicit instantiation is not a definition. After a
// template-id, clang reports the body or initializer (or a `,`) at the name and
// recovers as an explicit specialization (the `static_assert`s see it); after
// a plain declarator-id the `template` is ignored.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:17 error "explicit template instantiation cannot have a definition; if this definition is meant to be an explicit specialization, add '<>' after the 'template' keyword"
//@ diag: 32:24 error "explicit template instantiation cannot have a definition"
//@ diag: 33:24 error "explicit template instantiation cannot have a definition"
//@ diag: 34:24 error "explicit template instantiation cannot have a definition"
//@ diag: 35:31 error "explicit template instantiation cannot have a definition"
//@ diag: 36:31 error "explicit template instantiation cannot have a definition"
//@ diag: 37:24 error "explicit template instantiation cannot have a definition"
//@ diag: 38:14 error "explicit template instantiation cannot have a definition"
//@ diag: 38:21 error "an explicit template instantiation can only instantiate a single entity"
//@ diag: 39:28 error "function cannot be defined in an explicit instantiation; if this declaration is meant to be a function definition, remove the 'template' keyword"
//@ diag: 39:24 error "redefinition of 'h'"
//@ diag: 28:15 note "previous definition is here"
//@ diag: 40:26 error "variable cannot be defined in an explicit instantiation; if this declaration is meant to be a variable definition, remove the 'template' keyword"
//@ diag: 41:17 error "variable cannot be defined in an explicit instantiation"
//@ diag: 41:17 error "an explicit template instantiation can only instantiate a single entity"
//@ diag: 41:19 error "explicit instantiation of 'k' does not refer to a function template, variable template, member function, member class, or static data member"
template <class T> struct S { static constexpr int value = 0; };
template <class T> constexpr int f() { return 0; }
template <class T> constexpr int v = 0;
template <class T> int u = 0;
template <class T> constexpr int g();
constexpr int h() { return 0; }
constexpr int k() { return 0; }

template struct S<int> { static constexpr int value = 1; };
template constexpr int f<int>() { return 1; }
template constexpr int v<int> = 1;
extern template struct S<long> { static constexpr int value = 2; };
extern template constexpr int f<long>() { return 2; }
extern template constexpr int v<long> = 2;
template constexpr int f<short>() = delete;
template int u<char>, u<short>;
template constexpr int h() { return 0; }
template constexpr int w = 3;
template int h(), k();

static_assert(S<int>::value == 1 && f<int>() == 1 && v<int> == 1);
static_assert(S<long>::value == 2 && f<long>() == 2 && v<long> == 2);
static_assert(w == 3);
