// [over.best.ics]/10, [over.match.best]/3: an ambiguous user-defined
// conversion of an argument, and a deleted or inaccessible conversion
// function selected for it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:5 error "conversion function from 'Del' to 'long' invokes a deleted function"
//@ diag: 14:14 note "'operator int' has been explicitly marked deleted here"
//@ diag: 17:13 note "passing argument to parameter 'l' here"
//@ diag: 23:5 error "'operator int' is a private member of 'Priv'"
//@ diag: 15:14 note "implicitly declared private here"
//@ diag: 24:3 error "no matching function for call to 'y'"
//@ diag: 17:6 note "candidate function not viable: no known conversion from 'Ex' to 'long' for 1st argument"
struct M { operator int(); operator long(); };
struct Del { operator int() = delete; };
class Priv { operator int(); };
struct Ex { explicit operator int(); };
void y(long l);
void z(const long &l);
void t(M m, Del d, Priv p, Ex x) {
  y(m);
  z(m);
  y(d);
  y(p);
  y(x);
}
