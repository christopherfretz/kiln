// [expr.const.core]/2.32, [dcl.pre]/P3289R1 (spec/consteval-blocks.md §3):
// the evaluation of a consteval-block-declaration is the one context in
// which injection may happen ("plainly constant-evaluated" in P3289R1;
// the merged wording names the consteval block itself). The kiln
// extension `__kiln_plainly_constant_evaluated()` returns that bit; one
// check per row of the §3 table. A row that must be false uses
// `static_assert(!...)` or a consteval function that fails when the bit
// is set; a true row fails the block otherwise.
//@ expect: compile-ok
//@ skip(clang): kiln extension builtin; clang 19 has no consteval blocks
//@ strict-diags
int rt();

// Row 1: a consteval block's own evaluation (true), including the
// functions it calls (they are part of that evaluation).
consteval { if (!__kiln_plainly_constant_evaluated()) rt(); }
consteval bool plain_now() { return __kiln_plainly_constant_evaluated(); }
consteval { if (!plain_now()) rt(); }
// A memoized call's cached result is per bit (a callee reads it).
constexpr bool plain_leaf(int) { return __kiln_plainly_constant_evaluated(); }
constexpr bool plain_memo(int x) { return plain_leaf(x); }
static_assert(!plain_memo(1));
consteval { if (!plain_memo(1)) rt(); }
static_assert(!plain_memo(1));
struct InClass {
  consteval { if (!__kiln_plainly_constant_evaluated()) rt(); }
};
void in_block() {
  consteval { if (!__kiln_plainly_constant_evaluated()) rt(); }
}

// Row 2: an evaluation nested in a consteval block (a static_assert, a
// constexpr local's initializer) is its own evaluation (false).
consteval {
  static_assert(!__kiln_plainly_constant_evaluated());
  constexpr bool nested = __kiln_plainly_constant_evaluated();
  if (nested) rt();
}

// Row 3: a static_assert condition.
static_assert(!__kiln_plainly_constant_evaluated());

// Row 4: the initializer of a constexpr variable.
constexpr bool cx = __kiln_plainly_constant_evaluated();
static_assert(!cx);

// Row 5: a constinit (constant-initialized) variable.
constinit const bool ci = __kiln_plainly_constant_evaluated();
static_assert(!ci);

// Row 6: a const variable's constant initializer.
const bool cv = __kiln_plainly_constant_evaluated();
static_assert(!cv);

// Row 7: a template argument.
template <bool B> struct Arg { static constexpr bool value = B; };
static_assert(!Arg<__kiln_plainly_constant_evaluated()>::value);

// Row 8: an `if constexpr` condition.
constexpr int if_cx() {
  if constexpr (__kiln_plainly_constant_evaluated()) return 1;
  else return 0;
}
static_assert(if_cx() == 0);

// Row 9: an array bound.
char bound[__kiln_plainly_constant_evaluated() ? 1 : 2];
static_assert(sizeof(bound) == 2);

// Row 10: an enumerator initializer.
enum E { e0 = __kiln_plainly_constant_evaluated() };
static_assert(e0 == 0);

// Row 11: a noexcept-specifier operand.
void nf() noexcept(!__kiln_plainly_constant_evaluated());
static_assert(noexcept(nf()));

// Row 12: a bit-field width.
struct Bits { unsigned w : __kiln_plainly_constant_evaluated() ? 1 : 3; };
static_assert(Bits{5}.w == 5);

// Row 13: a case label.
constexpr int sw(int x) {
  switch (x) {
  case __kiln_plainly_constant_evaluated(): return 1;
  default: return 2;
  }
}
static_assert(sw(0) == 1);

// Row 14: an immediate invocation outside a consteval block.
consteval int not_plain() {
  if (__kiln_plainly_constant_evaluated()) throw 0;
  return 1;
}
int use_imm() { return not_plain(); }

// Row 15: a constraint (atomic constraint satisfaction).
template <class T>
  requires(!__kiln_plainly_constant_evaluated())
constexpr int constrained() { return 1; }
static_assert(constrained<int>() == 1);

// Row 16: the operand of __builtin_constant_p, a trial (false even in a
// consteval block: a true bit would make the operand non-constant).
consteval {
  if (!__builtin_constant_p(__kiln_plainly_constant_evaluated() ? rt() : 1)) rt();
}

// Row 17: a constexpr function evaluated in a constant expression.
constexpr bool cx_fn() { return __kiln_plainly_constant_evaluated(); }
static_assert(!cx_fn());

int main() { return use_imm() - 1; }
