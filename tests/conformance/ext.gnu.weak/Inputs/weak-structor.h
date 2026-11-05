// The classes of ../weak-structor-run.cpp, whose structors are defined
// weak there and strong in weak-structor-strong.cpp.
extern "C" int printf(const char *, ...);
struct A { A(int); ~A(); int x; };
struct V { virtual void key(); virtual ~V(); };
struct B0 { B0() {} };
struct VB : virtual B0 { VB(); };
struct K { K(); ~K(); };
template <class T> struct TO { TO(T); };
