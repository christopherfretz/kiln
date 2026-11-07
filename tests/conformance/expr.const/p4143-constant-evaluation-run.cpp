// [expr.const] as amended by P4143R0 (Croydon DR, "Constant evaluation
// fixes"), the paper's motivating example: a constexpr local variable of
// a recursive function is a distinct object in each invocation (its
// address differs per frame), so it is created at run time even though
// its initializer is a constant expression; and [expr.const]/5.1 note:
// std::is_constant_evaluated() is true while determining whether an
// initializer is constant-initializable, so a const int initialized with
// it is 1 and usable in a constant expression; int ga has constant
// initialization, so it is 1 too, while the dynamic initialization of
// the automatic int w sees false. if consteval follows the same
// manifestly-constant-evaluated rule.
//@ expect: run
//@ strict-diags
//@ stdout: 42 1 1
//@ stdout: 1 1 1 2 2
extern "C" int printf(const char *, ...);
namespace std {
constexpr bool is_constant_evaluated() noexcept { return __builtin_is_constant_evaluated(); }
} // namespace std
struct node { int data; node *next; };
const int *addrs[4];
node *search_for_meaning(node *head, int depth) {
  constexpr int answer = 42;
  addrs[depth] = &answer;
  if (!head || head->data == answer) return head;
  return search_for_meaning(head->next, depth + 1);
}
constexpr int g() { return std::is_constant_evaluated() ? 1 : 2; }
constexpr int k() {
  if consteval { return 1; } else { return 2; }
}
int ga = g();
const int gb = g();
static_assert(gb == 1);
int main() {
  node c{42, nullptr}, b{2, &c}, a{1, &b};
  node *r = search_for_meaning(&a, 0);
  printf("%d %d %d\n", r->data, addrs[0] != addrs[1], addrs[1] != addrs[2]);
  constexpr int x = k();
  const int y = k();
  int z = k();
  int w = g();
  printf("%d %d %d %d %d\n", ga, x, y, z, w);
}
