// [except.ctor]/3: when a constructor exits by an exception, the virtual
// bases the complete-object constructor already constructed are destroyed,
// in reverse order: after a member's constructor throws, after a later
// virtual base's constructor throws, and when the destructor of a temporary
// of the virtual base's own initializer throws once the base is complete
// (as for a non-virtual base, WP-1458). A member destructor that throws
// during that unwinding calls terminate ([except.terminate]/1.3).
// Found running GCC's C++ testsuite. clang 19 and 22 and GCC 16.1 leak the
// virtual base in the temporary cases (`V3`, `V5`) and the non-virtual base in
// the sibling (`V4`); the other lines match both.
//@ expect: run
//@ oracle: none
//@ xfail(clang): clang 19 and 22 do not destroy a base whose initializer's temporary throws
//@ stdout-file: run-virtual-base-unwind.stdout
extern "C" int printf(const char*, ...);
extern "C" void exit(int);
namespace std {
using terminate_handler = void (*)();
terminate_handler set_terminate(terminate_handler) noexcept;
}
struct T { ~T() noexcept(false) { printf("~T "); throw 2; } };
struct V { int i; V(int i) : i(i) { printf("V%d ", i); } ~V() { printf("~V%d ", i); } };
struct W { W(int) { printf("W "); throw 3; } };
struct M { M(int) { printf("M "); throw 1; } };
struct X { X() {} ~X() noexcept(false) { printf("~X "); throw 4; } };
struct D : virtual V { M m; D() : V(1), m(0) {} };
struct E : virtual V, virtual W { E() : V(2), W(0) {} };
struct F : virtual V { F() : V((T(), 3)) {} };
struct G : V { G() : V((T(), 4)) {} };
struct H : F { H() : V((T(), 5)) {} };
struct I : virtual V { X x; M m; I() : V(6), m(0) {} };
int main() {
  std::set_terminate([] { printf("terminate\n"); exit(0); });
  try { D d; } catch (int e) { printf("caught %d\n", e); }
  try { E e; } catch (int e) { printf("caught %d\n", e); }
  try { F f; } catch (int e) { printf("caught %d\n", e); }
  try { G g; } catch (int e) { printf("caught %d\n", e); }
  try { H h; } catch (int e) { printf("caught %d\n", e); }
  try { I i; } catch (int e) { printf("caught %d\n", e); }
}
