// [dcl.meaning.general]/2.3, [temp.friend]/1, [temp.over.link]/6: a qualified friend function
// template must redeclare a template of the nominated namespace. In a non-template class a
// mismatch is diagnosed at once; in a class template the match is deferred to instantiation
// (the friend's parameters are a level deeper), unless the name finds nothing at all. Like clang,
// an instantiated friend template that matches no template is not diagnosed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:47 error "friend declaration of 'a' does not match any declaration in namespace 'ns'"
//@ diag: 15:47 error "friend declaration of 'b' does not match any declaration in namespace 'ns'"
//@ diag: 18:38 error "friend declaration of 'nothere' does not match any declaration in namespace 'ns'"
namespace ns {
template <class U> void a(U);
template <class U> void b(U, int);
struct Q { template <class U> friend void ns::a(U*);
           template <class U> friend void ns::b(U, long); };
template <class T> struct pc {
  template <class U> friend void ns::a(U*);
  template <class U> friend void ns::nothere(U*);
  template <class U> friend void ns::b(U, T);
};
pc<long> x;
}
