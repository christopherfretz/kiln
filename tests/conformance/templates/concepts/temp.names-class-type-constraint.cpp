//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:1 error "constraints not satisfied for class template 'R' [with T = int]"
//@ diag: 8:11 note "because 'int' does not satisfy 'Ptr'"
//@ diag: 7:34 note "because '__is_pointer(int)' evaluated to false"
// [temp.names]/8: a constrained class template parameter.
template <class T> concept Ptr = __is_pointer(T);
template <Ptr T> struct R { T p; };
R<int> r;
