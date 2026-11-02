// [temp.arg.nontype]/1, [lex.icon]: an integer argument of an `auto`
// template parameter prints with its type's literal suffix (`A<5U>`,
// `A<5L>`, `A<5UL>`, `A<5LL>`, `A<5ULL>`, `A<-5L>`), plain for `int`, and as a
// cast for a type with no suffix (`A<(short)5>`), as clang prints them
// (WP-319, the WP-303 follow-up).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 25:1 warning "'A<5U>' is deprecated"
//@ diag: 24:28 note "'A<5U>' has been explicitly marked deprecated here"
//@ diag: 26:1 warning "'A<5L>' is deprecated"
//@ diag: 24:28 note "'A<5L>' has been explicitly marked deprecated here"
//@ diag: 27:1 warning "'A<5UL>' is deprecated"
//@ diag: 24:28 note "'A<5UL>' has been explicitly marked deprecated here"
//@ diag: 28:1 warning "'A<5LL>' is deprecated"
//@ diag: 24:28 note "'A<5LL>' has been explicitly marked deprecated here"
//@ diag: 29:1 warning "'A<5ULL>' is deprecated"
//@ diag: 24:28 note "'A<5ULL>' has been explicitly marked deprecated here"
//@ diag: 30:1 warning "'A<(short)5>' is deprecated"
//@ diag: 24:28 note "'A<(short)5>' has been explicitly marked deprecated here"
//@ diag: 31:1 warning "'A<5>' is deprecated"
//@ diag: 24:28 note "'A<5>' has been explicitly marked deprecated here"
//@ diag: 32:1 warning "'A<-5L>' is deprecated"
//@ diag: 24:28 note "'A<-5L>' has been explicitly marked deprecated here"
template <auto v> struct [[deprecated]] A {};
A<5u> a1;
A<5l> a2;
A<5ul> a3;
A<5ll> a4;
A<5ull> a5;
A<(short)5> a6;
A<5> a7;
A<-5l> a8;
