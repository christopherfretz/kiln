// [temp.inst], [dcl.init.aggr]/3.1: a designated initializer list in a
// template keeps its designators until instantiation; the semantic checks
// (order, unknown field, non-aggregate type, array designator into a
// class, override) run on the instantiated type at the written locations,
// while the syntactic C99-extension warnings fire once, on the pattern.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:36 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 30:44 note "first non-designated initializer is here"
//@ diag: 33:35 warning "nested designators are a C99 extension"
//@ diag: 34:35 warning "array designators are a C99 extension"
//@ diag: 29:52 warning "ISO C++ requires field designators to be specified in declaration order; field 'y' will be initialized after field 'x'"
//@ diag: 35:9 note "in instantiation of function template specialization 'g<Pt>' requested here"
//@ diag: 29:44 note "previous initialization for field 'y' is here"
//@ diag: 31:37 error "field designator 'q' does not refer to any field in type 'Pt'"
//@ diag: 36:8 note "in instantiation of function template specialization 'k<Pt>' requested here"
//@ diag: 32:29 error "initialization of non-aggregate type 'C' with a designated initializer list"
//@ diag: 37:7 note "in instantiation of function template specialization 'm<C>' requested here"
//@ diag: 32:29 error "initialization of non-aggregate type 'int' with a designated initializer list"
//@ diag: 38:9 note "in instantiation of function template specialization 'm<int>' requested here"
//@ diag: 33:52 warning "initializer overrides prior initialization of this subobject"
//@ diag: 39:7 note "in instantiation of function template specialization 'n<Q>' requested here"
//@ diag: 33:42 note "previous initialization is here"
//@ diag: 34:35 error "array designator cannot initialize non-array type 'Pt'"
//@ diag: 40:8 note "in instantiation of function template specialization 'o<Pt>' requested here"
struct Pt { int x; int y; int z; };
struct C { C(int); int x; };
struct Q { Pt p; int w; };
template<class T> T g(int v) { T a = {.y = 1, .x = v}; return a; }
template<class T> T h() { return T{.x = 1, 2}; }
template<class T> T k() { return T{.q = 1}; }
template<class T> T m() { T t{.x = 1}; return t; }
template<class T> T n() { return {.p.y = 1, .p.y = 2}; }
template<class T> T o() { return {[1] = 2}; }
int u = g<Pt>(2).x + h<Pt>().y;
Pt a = k<Pt>();
C b = m<C>();
int c = m<int>();
Q d = n<Q>();
Pt e = o<Pt>();
