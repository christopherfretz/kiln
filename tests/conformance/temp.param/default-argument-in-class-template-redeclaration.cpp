// [temp.param]/12: the default template arguments available are those of all
// prior declarations; a class template redeclaration that adds them makes them
// usable by later template-ids and by the definition
//@ expect: run
//@ strict-diags
template <class C, class T> class bs;
template <class C> struct tr { static constexpr int v = sizeof(C); };
template <class C, class T = tr<C> > class bs;
template <class C, class T> class bs {
public:
  int f() const { return T::v; }
};
static_assert(__is_same(bs<char>, bs<char, tr<char> >));
typedef bs<int> ibs;
template <class C, class T> int g(const bs<C, T>& b) { return b.f(); }
template int g<char, tr<char> >(const bs<char>&);
template <class A, class B> struct pr;
template <class A, class B = A> struct pr;
template <class A, class B> struct pr { B b; };
int main() {
  pr<short> p{2};
  return (ibs().f() == 4 && g(bs<char>()) == 1 && p.b == 2) ? 0 : 1;
}
