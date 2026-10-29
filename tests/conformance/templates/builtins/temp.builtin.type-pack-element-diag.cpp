// __type_pack_element (spec/sema-templates.md §9.3): the index must be in
// range.
// In an instantiation the column is the declarator's: kiln's types carry
// no argument locations yet (see spec/sema-templates.md §11, WP-074).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:29 error "a parameter pack may not be accessed at an out of bounds index"
//@ diag: 11 error "a parameter pack may not be accessed at an out of bounds index"
//@ diag: 12:9 note "in instantiation of template class 'Pick<2>' requested here"
typedef __type_pack_element<3, int, char, long> A;
template <unsigned long I> struct Pick { typedef __type_pack_element<I, short, double> type; };
Pick<2> p;
Pick<1>::type *ok;
