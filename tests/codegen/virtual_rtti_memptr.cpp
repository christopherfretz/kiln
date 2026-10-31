// RTTI of member-pointer types (`__pointer_to_member_type_info`: qualifier,
// incomplete-class and noexcept flags) and of noexcept function types and
// pointers to them (flag 64; the pointee is the function type without
// `noexcept`).
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char* name() const noexcept { return __name; }
protected:
  const char* __name;
};
}
struct S { int x; void f() noexcept; };
struct Inc;
namespace { struct Anon { int a; }; }
const char* m1() noexcept { return typeid(int S::*).name(); }
const char* m2() noexcept { return typeid(void (S::*)(int)).name(); }
const char* m3() noexcept { return typeid(const int S::*).name(); }
const char* m4() noexcept { return typeid(int Inc::*).name(); }
const char* m5() noexcept { return typeid(void (*)(int) noexcept).name(); }
const char* m6() noexcept { return typeid(void(int) noexcept).name(); }
const char* m7() noexcept { return typeid(void (S::*)() noexcept).name(); }
const void* m8() noexcept { return &typeid(int S::*); }
const char* m9() noexcept { return typeid(Inc* S::*).name(); }
const char* m10() noexcept { return typeid(int Anon::*).name(); }
const char* m11() noexcept { return typeid(void (**)() noexcept).name(); }
const char* m12() noexcept { return typeid(volatile long S::* const*).name(); }
