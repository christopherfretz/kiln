// [expr.prim.lambda.closure]/7, [temp.inst]/17: an unsatisfied constraint on
// a lambda's call operator in a template, generic (a type-constraint) and
// non-generic (a trailing requires-clause), found at instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:40 error "no matching function for call to object of type '(lambda at "
//@ diag: 19:9 note "in instantiation of function template specialization 'f<double>' requested here"
//@ diag: 16:40 note "candidate template ignored: constraints not satisfied [with x:auto = double]"
//@ diag: 16:43 note "because 'double' does not satisfy 'Small'"
//@ diag: 15:36 note "because 'sizeof(double) <= 4' (8 <= 4) evaluated to false"
//@ diag: 17:40 error "no matching function for call to object of type '(lambda at "
//@ diag: 20:9 note "in instantiation of function template specialization 'g<int>' requested here"
//@ diag: 17:40 note "candidate function not viable: constraints not satisfied"
//@ diag: 17:56 note "because 'sizeof(int) > 4' (4 > 4) evaluated to false"
template <class T> concept Small = sizeof(T) <= 4;
template <class T> int f(T v) { return [](Small auto x) { return 1; }(v); }
template <class T> int g(T v) { return [v]() requires (sizeof(T) > 4) { return 2; }(); }
template <class T> int h(T v) { return [v]() requires (sizeof(T) <= 4) { return 3; }(); }
int a = f(1.0);
int b = g(1);
int c = h(1);
