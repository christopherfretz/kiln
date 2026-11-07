// [expr.prim.splice]/2.5 (P3598R0), [expr.prim.id.unqual]/4 (P2900R14):
// in a contract predicate (pre, post, contract_assert), a splice that
// designates a variable or a structured binding declared outside the
// assertion has type const T, as an id-expression naming it does: a
// parameter, a namespace-scope variable, a local, a static data member, a
// structured binding, and a splice whose reflection is a template
// argument. Writing through it is an error. A variable of the predicate
// itself (a lambda's local) is not const-ified. GCC 16.1 rejects the same
// lines but the dependent splice in `t<^^g>`.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ strict-diags
//@ diag: 47:17 error "'this' argument to member function 'only_mut' has type 'const M', but function is not marked const"
//@ diag: 45:7 note "'only_mut' declared here"
//@ diag: 33:19 error "cannot modify 'p' in a contract predicate: it is implicitly const"
//@ diag: 33:48 error "cannot modify 'g' in a contract predicate: it is implicitly const"
//@ diag: 35:28 error "cannot modify 'l' in a contract predicate: it is implicitly const"
//@ diag: 36:19 error "cannot modify 's' in a contract predicate: it is implicitly const"
//@ diag: 38:28 error "cannot modify 'x' in a contract predicate: it is implicitly const"
//@ diag: 32:57 error "cannot modify 'g' in a contract predicate: it is implicitly const"
//@ diag: 40:3 note "in instantiation of function template specialization 't<^^g>' requested here"
#include <meta>
int g;
struct S {
  static int s;
};
struct P {
  int a, b;
};
template <std::meta::info R> void t() { contract_assert(++[:R:] > 0); }
void f(int p) pre(++[:^^p:] > 0) post(([:^^g:] = 1) > 0) {
  int l = 0;
  contract_assert(([:^^l:] += 1) > 0);
  contract_assert(++[:^^S::s:] > 0);
  auto [x, y] = P{1, 2};
  contract_assert(([:^^x:] = 3) > 0);
  contract_assert([] { int k = 0; return ++[:^^k:]; }() > 0);
  t<^^g>();
}
// A member function call through the splice takes a const object
// argument: a non-const member function is not viable.
struct M {
  int only_mut() { return 0; }
};
void h(M m) pre([:^^m:].only_mut() == 0) {}
