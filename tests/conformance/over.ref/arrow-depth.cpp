// [over.ref], [implimits]: an operator-> chain is cut off at depth 256
// with a backtrace (clang's -foperator-arrow-depth default).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:27 error "use of 'operator->' on type 'T<0>' would invoke a sequence of more than 256 'operator->' calls"
//@ diag: 16:35 note "'operator->' declared here produces an object of type 'T<0 + 1>'"
//@ diag: 16:35 note "'operator->' declared here produces an object of type 'T<1 + 1>'"
//@ diag: 16:35 note "'operator->' declared here produces an object of type 'T<2 + 1>'"
//@ diag: 16:35 note "'operator->' declared here produces an object of type 'T<3 + 1>'"
//@ diag: 16:35 note "(skipping 248 'operator->'s in backtrace)"
//@ diag: 16:35 note "'operator->' declared here produces an object of type 'T<252 + 1>'"
//@ diag: 16:35 note "'operator->' declared here produces an object of type 'T<253 + 1>'"
//@ diag: 16:35 note "'operator->' declared here produces an object of type 'T<254 + 1>'"
//@ diag: 16:35 note "'operator->' declared here produces an object of type 'T<255 + 1>'"
//@ diag: 17:27 note "use -foperator-arrow-depth=N to increase 'operator->' limit"
template<int K> struct T { T<K+1> operator->(); };
int a10(T<0> t) { return t->m; }
