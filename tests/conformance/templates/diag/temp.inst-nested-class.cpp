// [temp.inst]/3: a member class of a specialization is instantiated only
// when its definition is required; the note names it qualified.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:44 error "field has incomplete type 'void'"
//@ diag: 9:12 note "in instantiation of member class 'O<void>::I' requested here"
template <class T> struct O { struct I { T v; }; };
O<void> o;
O<void>::I i;
