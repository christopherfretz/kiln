// [temp.deduct.general]/7, [temp.inst]/11: explicit template arguments
// that make a member function template's parameter or return type
// ill-formed are a substitution failure (the candidate note names the
// substituted arguments), with no class-instantiation note since P<int>
// itself is well-formed; a default argument is not part of the immediate
// context, so it is a hard error under "in instantiation of default
// function argument expression". Inside a function template the chain
// adds the enclosing specialization.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:12 error "no matching function for call to 'f'"
//@ diag: 29:36 note "candidate template ignored: substitution failure [with c = <'a', 'b'>]: too many template arguments for class template 'A'"
//@ diag: 33:40 error "no matching function for call to 'g'"
//@ diag: 30:39 note "candidate template ignored: substitution failure [with c = <'a', 'b'>]: too many template arguments for class template 'A'"
//@ diag: 31:51 error "too many template arguments for class template 'A'"
//@ diag: 33:63 note "in instantiation of default function argument expression for 'h<'a', 'b'>' required here"
//@ diag: 27:26 note "template is declared here"
//@ diag: 34:33 error "no matching function for call to 'f'"
//@ diag: 35:13 note "in instantiation of function template specialization 'u<long>' requested here"
//@ diag: 29:36 note "candidate template ignored: substitution failure [with c = <'a', 'b'>]: too many template arguments for class template 'A'"
//@ diag: 34:71 error "no matching function for call to 'g'"
//@ diag: 30:39 note "candidate template ignored: substitution failure [with c = <'a', 'b'>]: too many template arguments for class template 'A'"
//@ diag: 31:51 error "too many template arguments for class template 'A'"
//@ diag: 34:104 note "in instantiation of default function argument expression for 'h<'a', 'b'>' required here"
//@ diag: 35:13 note "in instantiation of function template specialization 'u<long>' requested here"
//@ diag: 27:26 note "template is declared here"
template <char x> struct A {};
template <class... Ts> struct P {
  template <char... c> static void f(A<c...>, Ts...);
  template <char... c> static A<c...> g(Ts...);
  template <char... c> static void h(int = sizeof(A<c...>));
};
void t() { P<int>::f<'a', 'b'>({}, 1); P<int>::g<'a','b'>(1); P<int>::h<'a','b'>(); }
template<class... T> void u() { P<T...>::template f<'a', 'b'>({}, 1); P<T...>::template g<'a','b'>(1); P<T...>::template h<'a','b'>(); }
void t2() { u<long>(); }
