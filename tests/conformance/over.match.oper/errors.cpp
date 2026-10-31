// [over.match.oper]/10: no viable, ambiguous and deleted operator functions.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:5 error "overload resolution selected deleted operator '='"
//@ diag: 15:17 note "candidate function has been explicitly deleted"
//@ diag: 21:5 error "no viable overloaded '+='"
//@ diag: 16:15 note "candidate function not viable: no known conversion from 'const char[2]' to 'int' for 1st argument"
//@ diag: 22:5 error "no viable overloaded '-='"
//@ diag: 23:4 error "use of overloaded operator '[]' is ambiguous (with operand types 'U' and 'unsigned int')"
//@ diag: 17:16 note "candidate function"
//@ diag: 17:37 note "candidate function"
//@ diag: 24:3 error "call to object of type 'V' is ambiguous"
//@ diag: 18:16 note "candidate function"
//@ diag: 18:37 note "candidate function"
struct S { void operator=(const S&) = delete; };
struct T { T& operator+=(int); };
struct U { int operator[](int); int operator[](long); };
struct V { int operator()(int); int operator()(long); };
void f(S a, S b, T t, U u, V v) {
  a = b;
  t += "x";
  t -= 1;
  u[1u];
  v(1u);
}
