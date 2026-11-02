// [temp.names]/4: a member template-id of a dependent object needs the
// 'template' keyword; without it clang says "use 'template' keyword to
// treat 'f' as a dependent template name" (with a fix-it), whether the
// object is a parameter, a pointer, a member of the current instantiation
// or reached through 'this'. A non-dependent object needs none.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 17:42 error "use 'template' keyword to treat 'f' as a dependent template name"
//@ diag: 18:44 error "use 'template' keyword to treat 'f' as a dependent template name"
//@ diag: 21:54 error "use 'template' keyword to treat 'f' as a dependent template name"
//@ diag: 21:93 error "use 'template' keyword to treat 'g' as a dependent template name"
//@ diag: 22:42 error "use 'template' keyword to treat 'g' as a dependent template name"
//@ diag: 23:42 error "use 'template' keyword to treat 'f' as a dependent template name"
//@ diag: 24:60 error "use 'template' keyword to treat 'f' as a dependent template name"
struct A { template<class T> int f(int); int g(int); };
template<class T> int t1(T t) { return t.f<int>(1); }
template<class T> int t2(T *t) { return t->f<int>(1); }
template<class T> int t3(T t) { return t.template f<int>(1); }
template<class T> int t4() { A b; return b.f<int>(1); }
template<class T> struct C { T m; int k() { return m.f<int>(1); } int k2() { return this->m.g<int>(1); } };
template<class T> int t6(T t) { return t.g<int>; }
template<class T> int t7(T t) { return t.f<T>(1); }
template<class T> struct F { T m; int k() { return this->m.f<int>(1); } };
