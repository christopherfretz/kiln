// [class.member.lookup]/6, [over.match.oper]/3.1, [over.call.object]/1:
// an ambiguous lookup of a member operator through bases of different types.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 40:4 error "member 'operator()' found in multiple base classes of different types"
//@ diag: 31:17 note "member found by ambiguous name lookup"
//@ diag: 32:17 note "member found by ambiguous name lookup"
//@ diag: 41:5 error "member 'operator+' found in multiple base classes of different types"
//@ diag: 31:40 note "member found by ambiguous name lookup"
//@ diag: 32:43 note "member found by ambiguous name lookup"
//@ diag: 42:4 error "member 'operator[]' found in multiple base classes of different types"
//@ diag: 31:60 note "member found by ambiguous name lookup"
//@ diag: 32:66 note "member found by ambiguous name lookup"
//@ diag: 43:5 error "no viable overloaded '='"
//@ diag: 33:8 note "candidate function (the implicit copy assignment operator) not viable: no known conversion from 'int' to 'const O' for 1st argument"
//@ diag: 33:8 note "candidate function (the implicit move assignment operator) not viable: no known conversion from 'int' to 'O' for 1st argument"
//@ diag: 51:25 error "member 'operator==' found in multiple base classes of different types"
//@ diag: 48:18 note "member found by ambiguous name lookup"
//@ diag: 49:18 note "member found by ambiguous name lookup"
//@ diag: 51:35 error "member 'operator==' found in multiple base classes of different types"
//@ diag: 48:18 note "member found by ambiguous name lookup"
//@ diag: 49:18 note "member found by ambiguous name lookup"
//@ diag: 51:45 error "member 'operator==' found in multiple base classes of different types"
//@ diag: 48:18 note "member found by ambiguous name lookup"
//@ diag: 49:18 note "member found by ambiguous name lookup"
//@ diag: 37:34 error "member 'operator()' found in multiple base classes of different types"
//@ diag: 46:3 note "in instantiation of function template specialization 'tf<O>' requested here"
//@ diag: 31:17 note "member found by ambiguous name lookup"
//@ diag: 32:17 note "member found by ambiguous name lookup"
// (a using-declaration per base resolves the call)
struct A { void operator()(int) {} int operator+(int); int operator[](int); A& operator=(int); };
struct B { void operator()(double) {} int operator+(double); int operator[](double); B& operator=(double); };
struct O : A, B {};
struct A2 { void operator()(int) {} };
struct B2 { void operator()(double) {} };
struct O2 : A2, B2 { using A2::operator(); using B2::operator(); };
template<class T> void tf(T t) { t(1); }
void g() {
  O o;
  o(1);
  o + 1;
  o[1];
  o = 1;
  O2 o2;
  o2(1);
  tf(o);
}
struct C1 { bool operator==(int) const; };
struct C2 { bool operator==(double) const; };
struct C3 : C1, C2 {};
bool h(C3 c) { return c == 1 && 1 == c && c != 1; }
