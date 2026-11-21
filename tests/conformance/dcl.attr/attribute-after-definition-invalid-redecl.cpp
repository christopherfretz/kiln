// [clang ext] "attribute declaration must precede definition" (clang
// `checkNewAttributesAfterDef`, -Wignored-attributes) is also given on a
// redeclaration that an error leaves out of its entity's redeclaration
// chain, against the declaration it would have followed, as clang merges
// the attributes first: an `inline` function redeclaration after a
// non-inline definition, a `constexpr`/`consteval` mismatch, a variable
// or static data member redefinition, a redeclaration of another type,
// `static` or `thread_local` mismatches. So is [dcl.attr.noreturn]/1's
// "'noreturn' attribute does not appear on the first declaration". A
// function declared `static` after a non-static one is rejected before
// the attributes are merged: neither is given there, nor after a
// mismatched exception specification. spec/sema.md §4.5, WP-1840.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 93:16 warning "attribute declaration must precede definition"
//@ diag: 92:6 note "previous definition is here"
//@ diag: 93:44 error "inline declaration of 'f1' follows non-inline definition"
//@ diag: 92:6 note "previous definition is here"
//@ diag: 95:16 warning "attribute declaration must precede definition"
//@ diag: 94:6 note "previous definition is here"
//@ diag: 95:35 error "inline declaration of 'f2' follows non-inline definition"
//@ diag: 94:6 note "previous definition is here"
//@ diag: 98:16 warning "attribute declaration must precede definition"
//@ diag: 97:8 note "previous definition is here"
//@ diag: 98:30 error "redefinition of 's'"
//@ diag: 97:8 note "previous definition is here"
//@ diag: 100:16 warning "attribute declaration must precede definition"
//@ diag: 99:8 note "previous definition is here"
//@ diag: 100:31 error "redefinition of 't' with a different type: 'long' vs 'int'"
//@ diag: 99:8 note "previous definition is here"
//@ diag: 102:16 warning "attribute declaration must precede definition"
//@ diag: 101:9 note "previous definition is here"
//@ diag: 102:38 error "inline declaration of 'm' follows non-inline definition"
//@ diag: 101:9 note "previous definition is here"
//@ diag: 104:16 warning "attribute declaration must precede definition"
//@ diag: 103:5 note "previous definition is here"
//@ diag: 104:27 error "redefinition of 'v1'"
//@ diag: 103:5 note "previous definition is here"
//@ diag: 106:16 warning "attribute declaration must precede definition"
//@ diag: 105:5 note "previous definition is here"
//@ diag: 106:27 error "redefinition of 'v2'"
//@ diag: 105:5 note "previous definition is here"
//@ diag: 108:16 warning "attribute declaration must precede definition"
//@ diag: 107:5 note "previous definition is here"
//@ diag: 108:28 error "redefinition of 'v3' with a different type: 'long' vs 'int'"
//@ diag: 107:5 note "previous definition is here"
//@ diag: 110:16 warning "attribute declaration must precede definition"
//@ diag: 109:5 note "previous definition is here"
//@ diag: 110:34 error "static declaration of 'v4' follows non-static declaration"
//@ diag: 109:5 note "previous definition is here"
//@ diag: 112:16 warning "attribute declaration must precede definition"
//@ diag: 111:5 note "previous definition is here"
//@ diag: 112:40 error "thread-local declaration of 'v5' follows non-thread-local declaration"
//@ diag: 111:5 note "previous definition is here"
//@ diag: 112:40 error "redefinition of 'v5'"
//@ diag: 111:5 note "previous definition is here"
//@ diag: 114:16 warning "attribute declaration must precede definition"
//@ diag: 113:12 note "previous definition is here"
//@ diag: 114:27 error "redefinition of 'v6'"
//@ diag: 113:12 note "previous definition is here"
//@ diag: 116:3 error "'noreturn' attribute does not appear on the first declaration"
//@ diag: 115:6 note "previous declaration is here"
//@ diag: 116:3 warning "attribute declaration must precede definition"
//@ diag: 115:6 note "previous definition is here"
//@ diag: 116:26 error "inline declaration of 'h1' follows non-inline definition"
//@ diag: 115:6 note "previous definition is here"
//@ diag: 118:3 error "'noreturn' attribute does not appear on the first declaration"
//@ diag: 117:6 note "previous declaration is here"
//@ diag: 118:3 warning "attribute declaration must precede definition"
//@ diag: 117:6 note "previous definition is here"
//@ diag: 118:29 error "constexpr declaration of 'h2' follows non-constexpr declaration"
//@ diag: 117:6 note "previous declaration is here"
//@ diag: 120:16 warning "attribute declaration must precede definition"
//@ diag: 119:15 note "previous definition is here"
//@ diag: 120:27 error "non-constexpr declaration of 'ce' follows constexpr declaration"
//@ diag: 119:15 note "previous declaration is here"
//@ diag: 122:16 warning "attribute declaration must precede definition"
//@ diag: 121:6 note "previous definition is here"
//@ diag: 122:38 error "consteval declaration of 'ce2' follows non-constexpr declaration"
//@ diag: 121:6 note "previous declaration is here"
//@ diag: 124:35 warning "attribute declaration must precede definition"
//@ diag: 123:25 note "previous definition is here"
//@ diag: 124:54 error "inline declaration of 't1' follows non-inline definition"
//@ diag: 123:25 note "previous definition is here"
//@ diag: 128:35 error "static declaration of 'n1' follows non-static declaration"
//@ diag: 127:6 note "previous definition is here"
//@ diag: 130:26 error "static declaration of 'n2' follows non-static declaration"
//@ diag: 129:6 note "previous declaration is here"
//@ diag: 132:28 error "'n3' is missing exception specification 'noexcept'"
//@ diag: 131:6 note "previous declaration is here"
void f1() {}
__attribute__((always_inline)) inline void f1();
void f2() {}
__attribute__((cold)) inline void f2();
struct C { static int s; static int t; void m(); };
int C::s = 1;
__attribute__((used)) int C::s;
int C::t = 1;
__attribute__((used)) long C::t;
void C::m() {}
__attribute__((cold)) inline void C::m();
int v1 = 1;
__attribute__((used)) int v1 = 2;
int v2 = 1;
__attribute__((used)) int v2;
int v3 = 1;
__attribute__((used)) long v3;
int v4 = 1;
__attribute__((used)) static int v4;
int v5 = 1;
__attribute__((used)) thread_local int v5;
inline int v6 = 1;
__attribute__((used)) int v6;
void h1() {}
[[noreturn]] inline void h1();
void h2() {}
[[noreturn]] constexpr void h2();
constexpr int ce() { return 1; }
__attribute__((cold)) int ce();
void ce2() {}
__attribute__((cold)) consteval void ce2();
template <class T> void t1(T) {}
template <class T> __attribute__((cold)) inline void t1(T);

// No attribute check.
void n1() {}
__attribute__((cold)) static void n1();
void n2();
[[noreturn]] static void n2();
void n3() noexcept {}
__attribute__((cold)) void n3();
