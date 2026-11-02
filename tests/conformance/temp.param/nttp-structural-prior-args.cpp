// [temp.param]/7, [temp.arg.general]: a non-type parameter's type
// substituted from prior arguments must be structural; clang reports it at
// the parameter "while substituting prior template arguments", while a
// deduced placeholder type is checked at the argument (libc++ pair/array
// NTTP of non-structural element type).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:22 error "type 'Pair<string, string>' (aka 'Pair<Str, Str>') of non-type template parameter is not a structural type"
//@ diag: 21:11 note "while substituting prior template arguments into non-type template parameter 'V' [with T = Pair<string, string>]"
//@ diag: 16:48 note "'Pair<string, string>' (aka 'Pair<Str, Str>') is not a structural type because it has a non-static data member of non-structural type 'Str'"
//@ diag: 17:46 note "'Str' is not a structural type because it has a non-static data member that is not public"
//@ diag: 22:19 error "type 'Pair<Str, Str>' of non-type template parameter is not a structural type"
//@ diag: 16:48 note "'Pair<Str, Str>' is not a structural type because it has a non-static data member of non-structural type 'Str'"
//@ diag: 17:46 note "'Str' is not a structural type because it has a non-static data member that is not public"
//@ diag: 20:16 note "template parameter is declared here"
template <class T1, class T2> struct Pair { T1 first; T2 second; };
struct Str { constexpr Str() {} private: int len = 0; };
using string = Str;
template <class T, T V> struct Value {};
template <Pair P> struct Deduced {};
using A = Value<Pair<string, string>, Pair<string, string>{}>;
using B = Deduced<Pair<string, string>{}>;
template <class T, T V> struct Ok {};
using C = Ok<Pair<int, int>, Pair<int, int>{1, 2}>;
