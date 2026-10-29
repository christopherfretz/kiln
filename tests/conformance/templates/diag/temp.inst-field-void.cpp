// [temp.inst]/3, [class.mem.general]/7: instantiating the class
// definition instantiates member declarations; a field of type void is
// ill-formed and reported with the instantiation context.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:44 error "field has incomplete type 'void'"
//@ diag: 9:14 note "in instantiation of template class 'Holder<void>' requested here"
template <class T> struct Holder { T *p; T v; };
Holder<void> h;
