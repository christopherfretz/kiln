// [dcl.pre]/12 (P2741R3): the message of an assertion that holds is an
// unevaluated operand; with -Wno-invalid-static-assert-message (clang
// evaluates it by default) a non-constant message is fine. Dependent
// messages are checked only on instantiation.
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-invalid-static-assert-message
struct NotConst {
  unsigned long size() const { return 1; }
  const char* data() const { return "x"; }
};
struct View {
  const char* p;
  unsigned long n;
  constexpr const char* data() const { return p; }
  constexpr unsigned long size() const { return n; }
};
static_assert(true, NotConst{});
template <class T, class M> struct Holds { static_assert(sizeof(T) > 0, M{}); };
Holds<int, NotConst> h1;
Holds<char, View> h2;
template <class T> constexpr bool g() { static_assert(sizeof(T) >= 1, View{"g", 1}); return true; }
static_assert(g<long>(), View{"ok", 2});
int main() {}
