// [conv.ptr]/3, [dcl.init.ref]/5: the implicit derived-to-base pointer
// conversion and reference binding to a base instantiate a class template
// specialization named only as the source ([temp.inst]/2), also in
// overload resolution ([over.ics.rank]/4). Matches clang 19.
struct Base {
  int k = 3;
};
template <class T> struct Derived : Base {};
constexpr int f(const Base*) { return 1; }
constexpr int f(const void*) { return 0; }
static_assert(f(static_cast<Derived<int>*>(nullptr)) == 1);
constexpr const Base* b = static_cast<Derived<long>*>(nullptr);
static_assert(b == nullptr);
char g(const Base&);
long g(...);
Derived<char>& r();
static_assert(sizeof(g(r())) == 1);
template <class T> struct Wrap {
  Derived<T>* p;
};
static_assert(f(Wrap<short>{nullptr}.p) == 1);
