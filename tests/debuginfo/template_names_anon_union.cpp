// Debugger (WP-1400): a class template specialization with a default
// argument inside an inline namespace is named as clang names it
// (`n::v1::str<char, n::v1::traits<char> >`), a member of a block-scope
// anonymous union prints by name, and the function template instance
// whose defaulted argument goes through a member typedef is found by its
// canonical name.
//@ expect: debugger
//@ gdb: break template_names_anon_union.cpp:49
//@ gdb: run
//@ gdb: print s
//@ gdb: whatis s
//@ gdb: print i
//@ gdb: print fl == 0
//@ gdb: ptype pick<n::v1::str<char, n::v1::traits<char> >, n::v1::str<char, n::v1::traits<char> > >
//@ gdb-check: $1 = {c = 97 'a'}
//@ gdb-check: type = n::v1::str<char, n::v1::traits<char> >
//@ gdb-check: $2 = 5
//@ gdb-check: $3 = false
//@ gdb-check: type = int (n::v1::str<char, n::v1::traits<char> >)
//@ lldb: settings set target.disable-aslr false
//@ lldb: breakpoint set --file template_names_anon_union.cpp --line 49
//@ lldb: run
//@ lldb: frame variable s i
//@ lldb: image lookup -r -n "^pick<"
//@ lldb-check: stop reason = breakpoint 1.1
//@ lldb-check: s = (c = 'a')
//@ lldb-check: (int) i = 5
//@ lldb-check: int pick<n::v1::str<char, n::v1::traits<char>>, n::v1::str<char, n::v1::traits<char>>>(n::v1::str<char, n::v1::traits<char>>)
namespace n {
inline namespace v1 {
template <class T> struct traits {};
template <class C, class Tr = traits<C>> struct str {
  C c;
};
} // namespace v1
} // namespace n
template <bool B, class T = void> struct en {};
template <class T> struct en<true, T> {
  typedef T type;
};
template <class A, class = typename en<true, A>::type> int pick(A a) { return a.c + 1; }
int use(int k) {
  n::str<char> s{'a'};
  union {
    int i;
    float fl;
  };
  i = k;
  return pick(s) + i;
}
int main() { return use(5) == 0; }
