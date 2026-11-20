// [dcl.init.aggr]/3.1, [dcl.init.list]/3.1: a designated-initializer-list
// initializes an aggregate only. A nested designator (`.n.x`, the C99
// form clang accepts as an extension) reaches into the member's type, so
// that member must itself be an aggregate: designating into a class with
// a user-declared constructor, a private member or a virtual function is
// ill-formed, as `.n = {.x = 1}` is. kiln reports it, with clang's
// -Wc99-designator warning first; clang 19/22 accept it and initialize
// the subobject member by member (a deviation, docs/status/deviations.md).
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang accepts a nested designator into a non-aggregate member
//@ diag: 20:5 warning "nested designators are a C99 extension"
//@ diag: 20:7 error "initialization of non-aggregate type 'N' with a designated initializer list"
//@ diag: 23:5 warning "nested designators are a C99 extension"
//@ diag: 23:7 error "initialization of non-aggregate type 'P' with a designated initializer list"
//@ diag: 26:5 warning "nested designators are a C99 extension"
//@ diag: 26:7 error "initialization of non-aggregate type 'V' with a designated initializer list"
struct N { N(int); int x; };
struct S { N n; int y; };
S s{.n.x = 1};
struct P { private: int x; public: int z; };
struct Q { P p; };
Q q{.p.z = 1};
struct V { virtual void f(); int a; };
struct W { V v; };
W w{.v.a = 2};
