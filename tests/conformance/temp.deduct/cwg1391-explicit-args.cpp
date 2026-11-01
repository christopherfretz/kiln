// CWG1391, [temp.deduct.call]/1: a parameter made non-dependent by explicit
// template arguments is checked before the specialization is formed, so
// the candidate note says "candidate function template".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:12 error "no matching function for call to 'f'"
//@ diag: 16:24 note "candidate function template not viable: no known conversion from 'int' to 'int *' for 2nd argument"
//@ diag: 20:26 error "no matching function for call to 'h'"
//@ diag: 17:33 note "candidate function template not viable: no known conversion from 'int' to 'int *' for 3rd argument"
//@ diag: 20:43 error "no matching function for call to 'k'"
//@ diag: 18:24 note "candidate function template not viable: no known conversion from 'int' to 'int *' for 1st argument"
//@ diag: 20:54 error "no matching function for call to 'm'"
//@ diag: 19:24 note "candidate function template not viable: no known conversion from 'const char[2]' to 'int' for 2nd argument"
//@ diag: 20:70 error "no matching function for call to 'f'"
//@ diag: 16:24 note "candidate function template not viable: no known conversion from 'int' to 'int *' for 2nd argument"
template<class T> void f(T, int*);
template<class T, class U> void h(T, U, int*);
template<class T> void k(T*);
template<class T> void m(T, T);
void g() { f<int>(1, 2); h<int>(1, 2, 3); k<int>(1); m<int>(1, "a"); f(1, 2); }
