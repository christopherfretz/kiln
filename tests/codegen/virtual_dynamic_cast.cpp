// dynamic_cast (pointer down/cross/to void*/static up/final target) and
// typeid (polymorphic deref with bad_typeid check, type operand, pointer
// to fundamental RTTI, reference operand). typeid(*p) can throw, so `ti`
// is not noexcept (no EH before WP-100).
namespace std { class type_info; }
struct A { int a = 1; virtual ~A() noexcept {} };
struct B { long b = 2; virtual void fb() noexcept {} };
struct C : A, B { int c = 3; };
struct D final : C {};
C* down(A* a) noexcept { return dynamic_cast<C*>(a); }
B* cross(A* a) noexcept { return dynamic_cast<B*>(a); }
void* top(B* b) noexcept { return dynamic_cast<void*>(b); }
const void* ctop(const A* a) noexcept { return dynamic_cast<const void*>(a); }
A* up(C* c) noexcept { return dynamic_cast<A*>(c); }
D* fin(A* a) noexcept { return dynamic_cast<D*>(a); }
const std::type_info& ti(A* a) { return typeid(*a); }
const std::type_info& ts() noexcept { return typeid(C); }
const std::type_info& tp() noexcept { return typeid(const int*); }
const std::type_info& tr(A& a) noexcept { return typeid(a); }
