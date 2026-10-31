// [over.built]: selected built-in candidates convert class operands by
// their conversion functions ([over.match.oper]/11).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 36:5 error "no viable overloaded '='"
//@ diag: 25:8 note "candidate function (the implicit copy assignment operator) not viable: no known conversion from 'int' to 'const I' for 1st argument"
//@ diag: 25:8 note "candidate function (the implicit move assignment operator) not viable: no known conversion from 'int' to 'I' for 1st argument"
//@ diag: 38:12 error "use of overloaded operator '+' is ambiguous (with operand types 'D' and 'int')"
//@ diag: 38:12 note "because of ambiguity in conversion of 'D' to 'float'"
//@ diag: 27:12 note "candidate function"
//@ diag: 27:37 note "candidate function"
//@ diag: 38:12 note "built-in candidate operator+(float, int)"
//@ diag: 38:12 note "built-in candidate operator+(double, int)"
//@ diag: 38:12 note "built-in candidate operator+(long double, int)"
//@ diag: 38:12 note "built-in candidate operator+(int, int)"
//@ diag: 38:12 note "built-in candidate operator+(long, int)"
//@ diag: 38:12 note "built-in candidate operator+(long long, int)"
//@ diag: 38:12 note "built-in candidate operator+(__int128, int)"
//@ diag: 38:12 note "built-in candidate operator+(unsigned int, int)"
//@ diag: 38:12 note "built-in candidate operator+(unsigned long, int)"
//@ diag: 38:12 note "built-in candidate operator+(unsigned long long, int)"
//@ diag: 38:12 note "built-in candidate operator+(unsigned __int128, int)"
//@ diag: 45:13 error "invalid operands to binary expression ('SEC' and 'SEC')"
//@ diag: 46:12 error "invalid operands to binary expression ('Pt' and 'int')"
struct I { operator int&(); };
struct C { operator int() const; };
struct D { operator double() const; operator int() const; };
struct Pt { operator int*() const; };
struct V { operator volatile int*() const; };
struct M { int x; };
struct MP { operator int M::*() const; };
enum E { e0 }; enum class SE { a };
struct EC { operator E() const; };
struct SEC { operator SE() const; };
void f(I i, C c, D d, Pt p, V v, MP mp, M* pm, EC ec, SEC sc, int* ip) {
  i = 3; i += 2; ++i; i++; --i; i--; i <<= 1;
  (void)(c + 1); (void)(-c); (void)(~c); (void)(!c); (void)(c && c);
  (void)(d + 1);
  (void)(p[1]); (void)(1[p]); (void)(*p); (void)(p + 1); (void)(p - p);
  (void)(*v);
  (void)(pm->*mp); (void)(mp == mp);
  (void)(ec == ec); (void)(ec < e0); (void)(sc == sc); (void)(sc < sc);
  (void)(ip[c]);
  c = c;
  (void)(sc + sc);
  (void)(p * 2);
  (void)(v[0]);
}
