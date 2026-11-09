// [dcl.inline]/6 valid twin: inline after an inline (or constexpr, or
// in-class friend) definition, after a declaration that is not a
// definition, and a deleted function, are well-formed (WP-1042).
//@ expect: compile-ok
//@ strict-diags
//@ std: c++20
inline void a();
void a() {}
inline void a();
void b();
inline void b();
void b() {}
constexpr int c() { return 0; }
inline constexpr int c();
struct S { friend void fr() {} };
inline void fr();
void h() = delete;
inline void h();
template<class T> inline void t() {}
template<class T> inline void t();
void f2() {}
template<class T> struct C2 {
  friend void f2(T) {}
};
C2<int> c2;
