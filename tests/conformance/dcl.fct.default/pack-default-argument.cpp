// [dcl.fct.default]/3: a default argument shall not be specified for a
// function parameter pack. Diagnosed at the `=`, the argument discarded;
// a default argument that names an unexpanded pack, or is itself
// erroneous, reports that instead. Covers free functions, member
// function templates (whose default arguments are parsed late) and a
// generic lambda.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:36 error "parameter pack cannot have a default argument"
//@ diag: 20:36 error "parameter pack cannot have a default argument"
//@ diag: 22:38 error "parameter pack cannot have a default argument"
//@ diag: 23:38 error "parameter pack cannot have a default argument"
//@ diag: 24:40 error "use of undeclared identifier 'undeclared'"
//@ diag: 26:38 error "default argument contains unexpanded parameter pack 'T'"
//@ diag: 27:38 error "use of undeclared identifier 'undeclared2'"
//@ diag: 28:23 error "parameter pack cannot have a default argument"
//@ diag: 29:49 error "default argument contains unexpanded parameter pack 'T'"
//@ diag: 30:11 error "type 'int' of function parameter pack does not contain any unexpanded parameter packs"
template<class... T> void g(T... t = 0);
template<class... T> void h(T... t = {});
struct S {
  template<class... T> void f(T... t = 0);
  template<class... T> void g(T... t = {1});
  template<class... T> void h(T... t = undeclared);
};
template<class... T> void k(T... t = sizeof(T));
template<class... T> void m(T... t = undeclared2);
auto l = [](auto... a = 0) {};
template<class... T> struct C { void f(T... t = T()); };
void f(int... a = 0);
