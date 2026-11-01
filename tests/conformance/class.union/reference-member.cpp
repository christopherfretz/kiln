// [class.union.general]/2: a union has no non-static data member of
// reference type (clang `err_union_member_of_reference_type`, at the
// member's name), also in an anonymous union and when a class template
// specialization's member type is substituted.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:16 error "union member 'r' has reference type 'int &'"
//@ diag: 14:25 error "union member 'q' has reference type 'int &'"
//@ diag: 15:32 error "union member 'r' has reference type 'int &'"
//@ diag: 16:10 note "in instantiation of template class 'V<int &>' requested here"
//@ diag: 17:17 error "union member 'rr' has reference type 'int &&'"
//@ diag: 17:32 error "union member 'cr' has reference type 'const int &'"
union U { int &r; int i; };
struct S { union { int &q; }; };
template <class T> union V { T r; };
V<int &> v = {};
union X { int &&rr; const int &cr; };
