// __type_pack_element (spec/sema-templates.md §9.3), [temp.arg.nontype]/2,
// [expr.const]/12: a negative index is narrowed to size_t. clang reports
// the narrowing ([-Wc++11-narrowing], an error by default), then goes on
// with the converted index and reports it out of bounds too.
// In an instantiation kiln's column differs from clang's (kiln's types
// carry no argument locations yet; spec/sema-templates.md §11, WP-074).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:31 error "non-type template argument evaluates to -1, which cannot be narrowed to type '__size_t' (aka 'unsigned long') [-Wc++11-narrowing]"
//@ diag: 15:31 error "a parameter pack may not be accessed at an out of bounds index"
//@ diag: 16 error "non-type template argument evaluates to -2, which cannot be narrowed to type '__size_t' (aka 'unsigned long') [-Wc++11-narrowing]"
//@ diag: 17:7 note "in instantiation of template class 'P<-2>' requested here"
//@ diag: 16 error "a parameter pack may not be accessed at an out of bounds index"
//@ requires: clang>=22
using A = __type_pack_element<-1, int>;
template <int N> struct P { typedef __type_pack_element<N, int> type; };
P<-2> p;
