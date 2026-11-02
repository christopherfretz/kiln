// [expr.new]/24 (C++26, P2747R2): a placement new through a `void *`
// reloaded from memory constructs the subobject the `void *` was made
// from, and constructing an empty `[[no_unique_address]]` member or empty
// base leaves the member sharing its address intact; compile-time and
// run-time values agree.
//@ expect: run
//@ requires: clang>=22
//@ stdout: 7 9 4660 4660
//@ stdout: 7 9 4660 4660
extern "C" int printf(const char *, ...);
namespace std { using size_t = decltype(sizeof 0); }
void *operator new(std::size_t, void *p) noexcept;
struct F {};
struct G {};
struct S : F { [[no_unique_address]] G g; int x; };
struct M { int first; int second; };
struct H { void *p; };
constexpr int member(int k) { M m{1, 2}; H h{&m.first}; new (h.p) int(5 * k); return m.first + m.second; }
constexpr int via_pointer(int k) { M m{1, 2}; void *p = &m.second; void **pp = &p; new (*pp) int(9 * k); return m.second; }
constexpr int nua(int k) { S s{}; s.x = 0x1234 * k; H h{&s.g}; new (h.p) G(); return s.x; }
constexpr int empty_base(int k) { S s{}; s.x = 0x1234 * k; H h{static_cast<F *>(&s)}; new (h.p) F(); return s.x; }
constexpr int c[] = {member(1), via_pointer(1), nua(1), empty_base(1)};
volatile int one = 1;
int main() {
  printf("%d %d %d %d\n", c[0], c[1], c[2], c[3]);
  int k = one;
  printf("%d %d %d %d\n", member(k), via_pointer(k), nua(k), empty_base(k));
}
