// [temp.friend]/1, [temp.spec.partial.general]/9.3, [temp.names]/8,
// [temp.arg.general]/1: the checks of a class template's template-ids and
// partial specializations read the head of the declaration lookup finds,
// never that of a first declaration that is a friend inside a class
// template (deeper parameters, possibly other names): a partial
// specialization whose arguments are the primary's own parameters is
// diagnosed, an unsatisfied constraint names `C` and points at the
// definition's requires-clause, and the arity note points at the
// definition. clang 19's wording and locations (WP-299 audit).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:1 error "constraints not satisfied for class template 'cs' [with C = char]"
//@ diag: 23:30 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
//@ diag: 27:1 error "too many template arguments for class template 'two'"
//@ diag: 24:41 note "template is declared here"
//@ diag: 28:35 error "class template partial specialization does not specialize any template argument; to define the primary template, remove the template argument list"
//@ diag: 28:35 error "redefinition of 'two'"
//@ diag: 24:41 note "previous definition is here"
template <class I> class wrap {
  template <class X> requires (sizeof(X) > 1) friend class cs;
  template <class X, class Y> friend class two;
};
template <class C> requires (sizeof(C) > 1) class cs {};
template <class C, class D = int> class two {};
cs<short> ok;
cs<char> bad;
two<int, int, int> bad2;
template <class C, class D> class two<C, D> {};
