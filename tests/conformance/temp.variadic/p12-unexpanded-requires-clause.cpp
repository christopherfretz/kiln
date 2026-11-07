// [temp.variadic]/12, [temp.pre]/9: a requires-clause, or a concept's
// constraint-expression, that names a template parameter pack outside any
// pack expansion is ill-formed. clang diagnoses the whole clause at its
// begin ("requires clause contains unexpanded parameter pack"; for a
// concept definition, "expression contains ..."); a requires-expression
// inside the clause is reported as part of it (spec/sema-templates.md
// §8.2). WP-771.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:33 error "requires clause contains unexpanded parameter pack 'Ts'"
//@ diag: 19:33 error "requires clause contains unexpanded parameter pack 'Ts'"
//@ diag: 20:47 error "requires clause contains unexpanded parameter pack 'Ts'"
//@ diag: 21:36 error "expression contains unexpanded parameter pack 'Ts'"
//@ diag: 22:42 error "requires clause contains unexpanded parameter pack 'Ts'"
//@ diag: 23:31 error "requires clause contains unexpanded parameter pack 'Ns'"
//@ diag: 25:31 error "requires clause contains unexpanded parameter pack 'Ts'"
template <class T> concept Int = __is_same(T, int);
template <class... Ts> requires Int<Ts> void f(Ts...);
template <class... Ts> requires (sizeof(Ts) > 1) void g(Ts...);
template <class... Ts> void h(Ts...) requires Int<Ts>;
template <class... Ts> concept K = (sizeof(Ts) > 0);
template <class... Ts> void r() requires requires { requires sizeof(Ts) > 0; };
template <int... Ns> requires (Ns > 0) void n();
template <class... Ts> struct S {
  template <class U> requires Int<Ts> void m(U);
};
// Expanded packs are fine.
template <class... Ts> requires (Int<Ts> && ...) void ok1(Ts...);
template <class... Ts> requires (sizeof...(Ts) > 0) void ok2(Ts...);
template <class... Ts> concept ok3 = (Int<Ts> || ...);
template <class... Ts> void ok4() requires requires (Ts... xs) { (xs, ...); };
