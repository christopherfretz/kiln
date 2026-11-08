#include <memory>
#include <string>
struct B { virtual ~B() = default; int x = 0; };
struct D : B { int y = 1; };
int mem_sweep() {
  auto u = std::make_unique<int>(3);
  auto ua = std::make_unique<int[]>(4);
  auto s = std::make_shared<D>();
  std::shared_ptr<B> sb = s;
  auto sd = std::dynamic_pointer_cast<D>(sb);
  auto ss = std::static_pointer_cast<D>(sb);
  std::weak_ptr<B> w = sb;
  auto l = w.lock();
  std::allocator<std::string> a;
  std::string* p = a.allocate(2);
  std::construct_at(p, "hi");
  std::destroy_at(p);
  a.deallocate(p, 2);
  auto al = std::allocate_shared<int>(std::allocator<int>(), 5);
  std::unique_ptr<int, void(*)(int*)> ud(new int(1), [](int* q) { delete q; });
  int buf[4] = {1, 2, 3, 4};
  int out[4];
  std::uninitialized_copy(buf, buf + 4, out);
  std::uninitialized_fill(out, out + 4, 7);
  std::uninitialized_move(buf, buf + 4, out);
  std::ranges::uninitialized_copy(buf, out);
  auto ap = std::addressof(buf[0]);
  auto ta = std::to_address(ap);
  std::pointer_traits<int*>::pointer pt = ta;
  std::allocator_traits<std::allocator<int>>::pointer ap2 = pt;
  return *u + ua[0] + sd->y + ss->x + l->x + *al + *ud + out[0] + *ap2 + (s.use_count() > 0)
         + (sb.owner_before(w) ? 1 : 0) + (sb == nullptr);
}
