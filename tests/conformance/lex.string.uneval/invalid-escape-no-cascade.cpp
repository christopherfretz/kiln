// [lex.string.uneval]/1, [dcl.pre]/10, [dcl.attr.deprecated], [dcl.attr.nodiscard],
// [dcl.fct.def.delete], [dcl.link]: an unevaluated string literal holding a
// numeric escape (or an encoding prefix) is ill-formed, and the construct
// around it is dropped, so nothing cascades from it (clang
// `ParseUnevaluatedStringLiteralExpression`):
// no "static assertion failed", no deprecation or nodiscard warning from the
// dropped attribute, and `= delete("...")` still deletes (without a message).
// The escape is shown whole ('\x01b': hex escapes take every hex digit).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:24 error "invalid escape sequence '\\x01b' in an unevaluated string literal"
//@ diag: 32:28 error "invalid escape sequence '\\x01' in an unevaluated string literal"
//@ diag: 32:35 error "invalid escape sequence '\\x02' in an unevaluated string literal"
//@ diag: 33:34 error "invalid escape sequence '\\1' in an unevaluated string literal"
//@ diag: 34:61 error "invalid escape sequence '\\1' in an unevaluated string literal"
//@ diag: 36:38 error "invalid escape sequence '\\x7F' in an unevaluated string literal"
//@ diag: 37:16 error "invalid escape sequence '\\x01b' in an unevaluated string literal"
//@ diag: 38:19 error "invalid escape sequence '\\x01' in an unevaluated string literal"
//@ diag: 39:28 error "invalid escape sequence '\\x01' in an unevaluated string literal"
//@ diag: 40:15 error "invalid escape sequence '\\x01b' in an unevaluated string literal"
//@ diag: 41:21 error "invalid escape sequence '\\x01' in an unevaluated string literal"
//@ diag: 43:25 error "invalid escape sequence '\\x01' in an unevaluated string literal"
//@ diag: 44:27 error "invalid escape sequence '\\x01' in an unevaluated string literal"
//@ diag: 45:22 error "invalid escape sequence '\\x01b' in an unevaluated string literal"
//@ diag: 46:9 error "invalid escape sequence '\\x43' in an unevaluated string literal"
//@ diag: 47:9 error "invalid escape sequence '\\x43' in an unevaluated string literal"
//@ diag: 48:22 error "an unevaluated string literal cannot have an encoding prefix"
//@ diag: 49:14 error "an unevaluated string literal cannot have an encoding prefix"
//@ diag: 50:67 error "call to deleted function 'f5'"
//@ diag: 45:6 note "candidate function has been explicitly deleted"
static_assert(false, "a\x01b");
static_assert(false, "ok" "\x01" "\x02");
struct C { static_assert(false, "\1"); };
template<class T> struct D { static_assert(sizeof(T) == 0, "\1"); };
D<int> di;
void block() { static_assert(false, "\x7F"); }
[[deprecated("a\x01b")]] void f1();
[[deprecated("a" "\x01")]] void f2();
__attribute__((deprecated("\x01"))) void f3();
[[nodiscard("a\x01b")]] int f4();
struct [[nodiscard("\x01")]] E {};
E make();
namespace [[deprecated("\x01")]] N { int x; }
enum En { A [[deprecated("\x01")]] };
void f5() = delete("a\x01b");
extern "\x43" { void f6(); }
extern "\x43" int v;
static_assert(false, u8"ab");
[[deprecated(L"x")]] void f7();
void use() { f1(); f2(); f3(); f4(); make(); (void)N::x; (void)A; f5(); f7(); }
