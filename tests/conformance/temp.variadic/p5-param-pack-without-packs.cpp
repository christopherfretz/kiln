// [dcl.fct]/16, [temp.variadic]/5: the type of a function parameter pack
// must contain an unexpanded pack. clang prints the declared type (after
// recovery: a misplaced `auto` becomes `int`) at the ellipsis and drops
// the `...`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:12 error "type 'int' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 17:25 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 17:29 error "type 'int' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 18:29 error "type 'T' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 19:21 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 19:15 error "type 'int (*)(int)' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 20:22 error "type 'const char (&)[4]' of function parameter pack does not contain any unexpanded parameter packs"
//@ diag: 21:12 error "type 'int' of function parameter pack does not contain any unexpanded parameter packs"
template <class T> concept C = true;
void f1(int... x);
void f2(auto (*... x)(C auto...));
template <class T> void f3(T... x);
void f4(int (*...x)(auto));
void f5(const char (&... s)[4]);
void f6(int... x) {}
void ok1(C auto... x);
template <class... T> void ok2(T (*... x)(int));
void ok3(auto (*... x)(int));
