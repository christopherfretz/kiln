// [temp.variadic]/12 with P2841R7 template template parameter packs: a
// requires-clause naming a concept or variable template parameter pack
// outside a fold (`requires Cs<T>`), in a template head, a trailing clause,
// a member template or through a requires-expression, and a concept
// definition doing so, are ill-formed, diagnosed as clang 22 does
// (spec/sema-templates.md §2.1.1). WP-771.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:61 error "requires clause contains unexpanded parameter pack 'Cs'"
//@ diag: 19:61 error "requires clause contains unexpanded parameter pack 'Cs'"
//@ diag: 20:72 error "requires clause contains unexpanded parameter pack 'Cs'"
//@ diag: 21:55 error "expression contains unexpanded parameter pack 'Cs'"
//@ diag: 22:71 error "requires clause contains unexpanded parameter pack 'Cs'"
//@ diag: 23:82 error "requires clause contains unexpanded parameter pack 'Cs'"
//@ diag: 24:58 error "requires clause contains unexpanded parameter pack 'Vs'"
template <class T> concept Int = __is_same(T, int);
template <template <class> concept... Cs, class T> requires Cs<T> void f(T);
template <template <class> concept... Cs, class T> requires (Cs<T>) void f2(T);
template <template <class> concept... Cs, class T> void f3(T) requires Cs<T>;
template <template <class> concept... Cs> concept K = Cs<int>;
template <class T, template <class> concept... Cs> void f4() requires requires { requires Cs<T>; };
template <template <class> concept... Cs> struct S { template <class T> requires Cs<T> void h(T); };
template <template <class> auto... Vs, class T> requires Vs<T> void k(T);
// Folded packs are fine.
template <template <class> concept... Cs, class T> requires (Cs<T> && ...) void ok1(T);
template <template <class> auto... Vs, class T> requires (Vs<T> || ...) void ok2(T);
template <template <class> concept... Cs> concept ok3 = (Cs<int> && ...);
