// [meta.reflection.substitute] (P2996R13) can_substitute as an Oven trial
// (spec/oven.md "Checkpoints", spec/reflection.md §3.3): each substitution
// runs inside a Transactional checkpoint and a SFINAE frame, so a failing
// one leaves nothing behind. Ten thousand failing trials (unsatisfied
// constraints, a failing alias, a wrong argument count, a non-unique
// function specialization) give no diagnostic, no instantiation record
// (the golden's one row is the deduction of can_substitute's own range
// parameter at its five calls: no alias, signature-substitution or class
// row for the substituted templates, and none for Ok<int>, the one
// successful substitution, whose definition is never instantiated) and the
// same peak_bytes as one trial.
//@ expect: compile-ok
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ inst-report: substitute-trial-rollback.inst.json
//@ oven: 54 peak_bytes<=0 allocs<=0
//@ oven: 55 peak_bytes<=0 allocs<=0 steps<=70000
// Declared here rather than by kiln_meta.h so that the golden holds only
// this file's templates; the arguments are arrays.
namespace std::meta {
using info = decltype(^^int);
template <class R> consteval bool can_substitute(info templ, R&& arguments);
}  // namespace std::meta
using std::meta::can_substitute;
using std::meta::info;

template <class T, class U = T> struct Pair {};
template <class T> requires(sizeof(T) > 1) struct R {};
template <class T> using Bad = typename T::type;
template <class T, class U> void h(T, U) {}
template <class T> struct Ok { static_assert(sizeof(T) == 100); };

constexpr info chr[] = {^^char};
constexpr info i32[] = {^^int};
struct Span { const info* data; decltype(sizeof 0) size; };
constexpr Span none{nullptr, 0};

consteval int trial(int n) {
  int ok = 0;
  for (int k = 0; k < n; ++k) {
    ok += can_substitute(^^R, chr);
    ok += can_substitute(^^Bad, i32);
    ok += can_substitute(^^Pair, none);
    ok += can_substitute(^^h, i32);
    ok += can_substitute(^^Ok, i32);
  }
  return ok;
}

// Ok<int> is substitutable (its definition is never instantiated); the
// other four fail every time, and the memo keeps only successes, so every
// failure is a fresh trial.
static_assert(trial(1) == 1);
static_assert(trial(10000) == 10000);

int main() {}
