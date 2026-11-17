// [dcl.fct.default]/1, [dcl.init.list]/3.5: the rejected side of
// dependent-braced-default-after-temporary-cxx14.cpp. The default `= {}`
// still copy-list-initializes the substituted parameter type when it is
// used, so an explicit default constructor is an error there, as in clang.
//@ args: -std=c++14
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:61 error "chosen constructor is explicit in copy-initialization"
//@ diag: 20:9 note "in instantiation of default function argument expression for 'f<0>' required here"
//@ diag: 15:22 note "explicit constructor declared here"
//@ diag: 19:61 note "passing argument to parameter here"
template <bool B, class T> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
struct Nil {};
struct Ex { explicit Ex() {} };
constexpr bool truefn(Nil) { return true; }

template <int N = 0>
int f(typename enable_if<truefn(Nil{}) && N == 0, Ex>::type = {}) { return 1; }
int x = f();
