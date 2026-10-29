// [temp.inst]/16, [implimits]: recursion through a static data member's
// initializer (sizeof requires a complete type, [expr.sizeof]/1) hits the
// same depth limit.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:50 error "recursive template instantiation exceeded maximum depth of 1024"
//@ diag: 19:50 note "in instantiation of template class 'R<1024>' requested here"
//@ diag: 19:50 note "in instantiation of template class 'R<1023>' requested here"
//@ diag: 19:50 note "in instantiation of template class 'R<1022>' requested here"
//@ diag: 19:50 note "in instantiation of template class 'R<1021>' requested here"
//@ diag: 19:50 note "in instantiation of template class 'R<1020>' requested here"
//@ diag: 19:50 note "(skipping 1015 contexts in backtrace; use -ftemplate-backtrace-limit=0 to see all)"
//@ diag: 19:50 note "in instantiation of template class 'R<4>' requested here"
//@ diag: 19:50 note "in instantiation of template class 'R<3>' requested here"
//@ diag: 19:50 note "in instantiation of template class 'R<2>' requested here"
//@ diag: 19:50 note "in instantiation of template class 'R<1>' requested here"
//@ diag: 20:9 note "in instantiation of template class 'R<0>' requested here"
//@ diag: 19:50 note "use -ftemplate-depth=N to increase recursive template instantiation depth"
template <int N> struct R { static const int v = sizeof(R<N + 1>); };
int x = R<0>::v;
