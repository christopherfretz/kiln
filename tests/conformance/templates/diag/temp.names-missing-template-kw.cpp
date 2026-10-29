//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:36 error "use 'template' keyword to treat 'g' as a dependent template name"
//@ diag: 9:34 error "use 'template' keyword to treat 'g' as a dependent template name"
//@ diag: 10:36 error "missing 'template' keyword prior to dependent template name 'h'"
//@ diag: 11:38 error "use 'template' keyword to treat 'k' as a dependent template name"
// [temp.names]/3: a dependent member template needs the `template` keyword
template <class T> void f(T t) { t.g<int>(); }
template <class T> void h() { T::g<int>(); }
template <class T> void i(T t) { t.h<1>(); }
template <class T> void j(T *t) { t->k<>(); }
template <class T> bool ok(T t) { return t.x < 1 && T::y < 2; }
template <class T> bool ok2(T t) { return t.template g<int>() < T::template h<1>(); }
