// [expr.dynamic.cast], [expr.typeid]: dynamic_cast and typeid at run time
// through the C++ runtime's __dynamic_cast and the RTTI kiln emits
// (WP-080): downcasts, cross casts, casts to void*, failing casts, and the
// dynamic type's type_info (std::type_info declared by hand: no headers).
//@ expect: run
//@ stdout: down 1 cross 1 fail 1 void 1
//@ stdout: ref 6
//@ stdout: 1C 1C 1A PK1C 1E
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char *name() const noexcept { return __name; }
protected:
  const char *__name;
};
} // namespace std
extern "C" int printf(const char *, ...);
struct A { int a = 1; virtual ~A() {} };
struct B { long b = 2; virtual void fb() {} };
struct C : A, B { int c = 6; };
struct D : A {};
enum E { e0 };
int main() {
  C c;
  D d;
  A *pa = &c;
  B *pb = &c;
  A *pd = &d;
  printf("down %d cross %d fail %d void %d\n", dynamic_cast<C *>(pa) == &c,
         dynamic_cast<B *>(pa) == pb, dynamic_cast<C *>(pd) == nullptr,
         dynamic_cast<void *>(pb) == static_cast<void *>(&c));
  C &rc = dynamic_cast<C &>(*pb);
  printf("ref %d\n", rc.c);
  printf("%s %s %s %s %s\n", typeid(*pa).name(), typeid(*pb).name(), typeid(A).name(),
         typeid(const C *).name(), typeid(E).name());
}
