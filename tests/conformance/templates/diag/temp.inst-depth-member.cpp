// [temp.inst]/16, [implimits]: unbounded recursive instantiation through a
// member of the specialization's own type stops at the depth limit (1024
// by default) with a fatal error and a backtrace of the first 5 and last 5
// contexts.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:38 error "recursive template instantiation exceeded maximum depth of 1024"
//@ diag: 20:38 note "in instantiation of template class 'R<1024>' requested here"
//@ diag: 20:38 note "in instantiation of template class 'R<1023>' requested here"
//@ diag: 20:38 note "in instantiation of template class 'R<1022>' requested here"
//@ diag: 20:38 note "in instantiation of template class 'R<1021>' requested here"
//@ diag: 20:38 note "in instantiation of template class 'R<1020>' requested here"
//@ diag: 20:38 note "(skipping 1015 contexts in backtrace; use -ftemplate-backtrace-limit=0 to see all)"
//@ diag: 20:38 note "in instantiation of template class 'R<4>' requested here"
//@ diag: 20:38 note "in instantiation of template class 'R<3>' requested here"
//@ diag: 20:38 note "in instantiation of template class 'R<2>' requested here"
//@ diag: 20:38 note "in instantiation of template class 'R<1>' requested here"
//@ diag: 21:6 note "in instantiation of template class 'R<0>' requested here"
//@ diag: 20:38 note "use -ftemplate-depth=N to increase recursive template instantiation depth"
template <int N> struct R { R<N + 1> next; };
R<0> r;
