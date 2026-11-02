// [dcl.meaning.general]/2.3, [temp.friend]/1: a qualified friend function whose type depends on
// the enclosing class template's parameter is matched against the nominated namespace when the
// class is instantiated, and diagnosed then (clang adds a candidate note kiln does not emit).
//@ expect: compile-fail
//@ diag: 10:19 error "friend declaration of 'c' does not match any declaration in namespace 'ns'"
//@ diag: 13:10 note "in instantiation of template class 'ns::pc<long>' requested here"
namespace ns {
void c(int);
template <class T> struct pc {
  friend void ns::c(T);
};
pc<int> ok;
pc<long> bad;
}
