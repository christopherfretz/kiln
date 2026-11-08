// [temp.variadic]/5, /12: in a lambda's declarator the packs the lambda
// declares (its template parameter packs, its function parameter packs)
// must be expanded where they are named: a requires-clause naming one is
// "requires clause contains unexpanded parameter pack" at the clause's
// begin, a template parameter's default argument "default argument
// contains ..." (clang's `DiagnoseUnexpandedParameterPacks` with the
// lambda's `LocalPacks`). An enclosing template's pack named there makes
// the whole lambda-expression contain it, diagnosed at the enclosing
// full-expression or initializer when no pack expansion expands it. WP-776.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:12 error "initializer contains unexpanded parameter pack 'Ts'"
//@ diag: 26:3 error "expression contains unexpanded parameter pack 'Ts'"
//@ diag: 27:44 error "requires clause contains unexpanded parameter pack 'Us'"
//@ diag: 28:37 error "requires clause contains unexpanded parameter pack 'Us'"
//@ diag: 29:36 error "requires clause contains unexpanded parameter pack 'xs'"
//@ diag: 30:12 error "initializer contains unexpanded parameter pack 'Ts'"
//@ diag: 31:12 error "initializer contains unexpanded parameter pack 'Ts'"
//@ diag: 32:34 error "default argument contains unexpanded parameter pack 'Us'"
//@ diag: 33:36 error "default argument contains unexpanded parameter pack 'Us'"
//@ diag: 34:12 error "initializer contains unexpanded parameter pack 'Ts'"
template <class T> concept Int = __is_same(T, int);
template <class T, class U> concept Same = __is_same(T, U);
template <class... Ts> void bad() {
  auto a = []<class U>(U) requires Same<U, Ts> {};
  []<class U> requires Same<U, Ts> (U) {};
  auto c = []<class... Us>(Us...) requires Int<Us> {};
  auto d = []<class... Us> requires Int<Us> (Us...) {};
  auto e = [](auto... xs) requires (sizeof(xs) > 0) {};
  auto f = [] { return []() requires Int<Ts> { return 0; }; };
  auto g = []<class... Us>(Us...) requires (Int<Us> && ...) && Int<Ts> {};
  auto h = []<class... Us, class V = Us>(V) {};
  auto i = []<class... Us, int N = sizeof(Us)>() {};
  auto j = []<class U = Ts>() {};
}
template <class... Ts> void good() {
  ([]<class... Us>(Us...) requires (Int<Us> && ...) && Int<Ts> {}, ...);
  ([]<class U = Ts>() requires Int<Ts> {}, ...);
}
