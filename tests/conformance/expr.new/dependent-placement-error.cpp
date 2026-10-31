// [expr.new]/19: the placement arguments of a dependent new-expression
// select the allocation function at instantiation.
//@ expect: compile-fail
//@ diag: 8:43 error "no matching function for call to 'operator new'"
//@ diag: 9:19 note "in instantiation of function template specialization 'place<int>'"
//@ diag: 7:7 note "no known conversion from 'int' to 'void *'"
void *operator new(unsigned long, void *p) noexcept;
template <class T> T *place(T x) { return new (x) T; }
int *g() { return place(1); }
