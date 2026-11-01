// [cstdarg.syn]/1, C 7.16.1.1 (clang `BuildVAArgExpr`): the first operand
// of `__builtin_va_arg` must be a `va_list` lvalue; the type must be
// complete, non-abstract and POD (`-Wnon-pod-varargs`, an error by
// default; a reference type is non-POD); a type that undergoes default
// argument promotion is a `-Wvarargs` warning, deferred to the end of the
// function body as a runtime-behavior diagnostic (dropped in an
// unevaluated operand, emitted at once at namespace scope, re-checked at
// instantiation). The result is a prvalue. Constant evaluation rejects
// `va_arg` with the generic note.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu
//@ diag: 43:30 error "second argument to 'va_arg' is of incomplete type 'Inc'"
//@ diag: 35:8 note "forward declaration of 'Inc'"
//@ diag: 44:30 error "second argument to 'va_arg' is of abstract type 'Abs'"
//@ diag: 36:27 note "unimplemented pure virtual method 'f' in 'Abs'"
//@ diag: 45:30 error "second argument to 'va_arg' is of non-POD type 'NP'"
//@ diag: 52:26 error "non-const lvalue reference to type '__builtin_va_list' cannot bind to a value of unrelated type 'int'"
//@ diag: 53:33 error "second argument to 'va_arg' is of non-POD type 'int &'"
//@ diag: 53:8 error "non-const lvalue reference to type 'int' cannot bind to a temporary of type 'int'"
//@ diag: 57:15 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 58:12 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 46:30 warning "second argument to 'va_arg' is of promotable type 'char'; this va_arg has undefined behavior because arguments will be promoted to 'int'"
//@ diag: 47:30 warning "second argument to 'va_arg' is of promotable type 'float'; this va_arg has undefined behavior because arguments will be promoted to 'double'"
//@ diag: 48:30 warning "second argument to 'va_arg' is of promotable type 'bool'; this va_arg has undefined behavior because arguments will be promoted to 'int'"
//@ diag: 49:30 warning "second argument to 'va_arg' is of promotable type 'E'; this va_arg has undefined behavior because arguments will be promoted to 'int'"
//@ diag: 51:30 warning "second argument to 'va_arg' is of promotable type 'unsigned short'; this va_arg has undefined behavior because arguments will be promoted to 'int'"
//@ diag: 55:30 warning "second argument to 'va_arg' is of promotable type 'wchar_t'; this va_arg has undefined behavior because arguments will be promoted to 'unsigned int'"
//@ diag: 56:30 warning "second argument to 'va_arg' is of promotable type 'char16_t'; this va_arg has undefined behavior because arguments will be promoted to 'int'"
//@ diag: 68:15 error "static assertion expression is not an integral constant expression"
//@ diag: 65:18 note "subexpression not valid in a constant expression"
//@ diag: 68:15 note "in call to 'k(false)'"
//@ diag: 70:32 warning "second argument to 'va_arg' is of promotable type 'char'; this va_arg has undefined behavior because arguments will be promoted to 'int'"
//@ diag: 61:76 warning "second argument to 'va_arg' is of promotable type 'char'; this va_arg has undefined behavior because arguments will be promoted to 'int'"
//@ diag: 62:39 note "in instantiation of function template specialization 'g<char>' requested here"
struct Inc;
struct Abs { virtual void f() = 0; };
struct NP { NP(const NP&); int x; };
enum E : short { e0 };
enum class SE { a };
void f(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  (void)__builtin_va_arg(ap, Inc);
  (void)__builtin_va_arg(ap, Abs);
  (void)__builtin_va_arg(ap, NP);
  (void)__builtin_va_arg(ap, char);
  (void)__builtin_va_arg(ap, float);
  (void)__builtin_va_arg(ap, bool);
  (void)__builtin_va_arg(ap, E);
  (void)__builtin_va_arg(ap, SE);
  (void)__builtin_va_arg(ap, unsigned short);
  (void)__builtin_va_arg(n, int);
  int &r = __builtin_va_arg(ap, int &);
  (void)__builtin_va_arg(ap, int[2]);
  (void)__builtin_va_arg(ap, wchar_t);
  (void)__builtin_va_arg(ap, char16_t);
  (void)sizeof(__builtin_va_arg(ap, short));
  decltype(__builtin_va_arg(ap, unsigned char)) d = 0;
  __builtin_va_end(ap);
}
template <class T> T g(__builtin_va_list ap) { return __builtin_va_arg(ap, T); }
int gi(__builtin_va_list ap) { return g<char>(ap) + g<int>(ap); }
constexpr int k(bool b, ...) {
  __builtin_va_list ap;
  return b ? 0 : __builtin_va_arg(ap, int);
}
static_assert(k(true) == 0);
static_assert(k(false) == 0);
__builtin_va_list gl;
char gc = __builtin_va_arg(gl, char);
