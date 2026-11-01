//@ expect: ast
// [expr.new]/1,3: a parenthesized type-id after `new` (and after a
// new-placement) is a full type-id, so it may hold a function or
// member-pointer declarator; the unparenthesized new-type-id's
// ptr-operators (member pointers too) are greedy.
inline void *operator new(unsigned long, void *p) noexcept { return p; }
struct T { void f(); int m; };
using PMF = void (T::*)();
void g(void *buf) {
  auto a = new (void (T::*)())();
  auto b = new (int T::*)(&T::m);
  auto c = new (void (*)(int))(nullptr);
  auto d = new (int (*)[3]);
  auto e = new int T::*(&T::m);
  auto f = new (int (*[2])());
  auto h = new (void (T::*[2])());
  auto i = new (PMF)(&T::f);
  auto j = new int T::**;
  auto k = new (buf) (void (T::*)())(&T::f);
}
