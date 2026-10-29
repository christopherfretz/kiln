// __make_integer_seq (spec/sema-templates.md §9.3): the length must be
// non-negative.
// In an instantiation the column is the declarator's: kiln's types carry
// no argument locations yet (see spec/sema-templates.md §11, WP-074).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:30 error "integer sequences must have non-negative sequence length"
//@ diag: 12 error "integer sequences must have non-negative sequence length"
//@ diag: 13:7 note "in instantiation of template class 'M<-2>' requested here"
template <class T, T... Is> struct Seq {};
__make_integer_seq<Seq, int, -1> *p;
template <int N> struct M { typedef __make_integer_seq<Seq, int, N> type; };
M<-2> q;
M<2>::type *ok;
