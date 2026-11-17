// [implimits], [temp.inst]/16: GCC's `-ftemplate-depth-N` spelling sets
// the same limit as `-ftemplate-depth=N` (clang accepts both). Found
// running GCC's C++ testsuite (WP-1553).
//@ args: -ftemplate-depth-5
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:50 error "recursive template instantiation exceeded maximum depth of 5"
//@ diag: 15:50 note "in instantiation of template class 'R<5>' requested here"
//@ diag: 15:50 note "in instantiation of template class 'R<4>' requested here"
//@ diag: 15:50 note "in instantiation of template class 'R<3>' requested here"
//@ diag: 15:50 note "in instantiation of template class 'R<2>' requested here"
//@ diag: 15:50 note "in instantiation of template class 'R<1>' requested here"
//@ diag: 16:9 note "in instantiation of template class 'R<0>' requested here"
//@ diag: 15:50 note "use -ftemplate-depth=N to increase recursive template instantiation depth"
template <int N> struct R { static const int v = sizeof(R<N + 1>); };
int x = R<0>::v;
