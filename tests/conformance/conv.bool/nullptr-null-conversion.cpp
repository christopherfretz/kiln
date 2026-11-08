// [conv.bool]/1, [expr.unary.op]/9, [expr.log.and]/1: a `std::nullptr_t`
// operand contextually converted to `bool` (`!nullptr`, `nullptr && b`,
// `if (nullptr)`, each arm of a `nullptr` conditional) is valid and false;
// clang's -Wnull-conversion (on by default) warns about it, as it does
// for GNU `__null`, also for a non-constant `std::nullptr_t` operand, and
// not when the `nullptr` comes from a macro expanded in a different
// context (the last line). WP-647 (5); WP-851.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 21:16 warning "implicit conversion of nullptr constant to 'bool'"
//@ diag: 22:17 warning "implicit conversion of nullptr constant to 'bool'"
//@ diag: 23:11 warning "implicit conversion of nullptr constant to 'bool'"
//@ diag: 25:8 warning "implicit conversion of nullptr constant to 'bool'"
//@ diag: 26:12 warning "implicit conversion of nullptr constant to 'bool'"
//@ diag: 27:7 warning "implicit conversion of nullptr constant to 'bool'"
//@ diag: 28:10 warning "implicit conversion of nullptr constant to 'bool'"
//@ diag: 29:13 warning "implicit conversion of nullptr constant to 'bool'"
//@ diag: 30:20 warning "implicit conversion of nullptr constant to 'bool'"
//@ diag: 30:30 warning "implicit conversion of nullptr constant to 'bool'"
typedef decltype(nullptr) np;
static_assert(!nullptr, "");
static_assert(!(nullptr || false), "");
bool a = !nullptr;
void f(np p) {
  if (!nullptr) {}
  bool d = nullptr && true;
  if (nullptr) {}
  bool e(nullptr);
  bool g = !p;
  bool h = (true ? nullptr : nullptr) || false;
  (void)d; (void)e; (void)g; (void)h;
}
#define N nullptr
bool i = !N;
int *q = nullptr;
