// [temp.explicit]/12, [class.friend]/6: a friend function is not a member,
// so an explicit instantiation declaration of the class template
// specialization that declares it does not cover it. A function first
// declared by the friend declaration of such a specialization and defined
// at namespace scope is an ordinary external definition; one defined in
// the class is inline and emitted where used (found building libc++ with
// kiln: `std::__get_ostream_file`, first declared as a friend of
// `basic_filebuf<char>`, was not emitted; WP-1620). Compared with clang's
// IR exactly.
//@ expect: ir
namespace s {
struct os;
template <class T> struct fb {
  friend int *gof(os &);
  friend int inl(fb *p) { return p->v; }
  int *f;
  int v;
};
extern template struct fb<char>;
int *gof(os &) { return nullptr; }
int use(fb<char> *p) { return inl(p); }
} // namespace s
