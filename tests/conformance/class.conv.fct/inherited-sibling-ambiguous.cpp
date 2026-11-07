// [class.conv.fct]/9, [over.match.copy]/1.2, [over.match.best]/3: two
// unrelated bases' conversion functions to the same type are both
// candidates (neither hides the other), so the conversion is ambiguous.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:5 error "conversion from 'LR' to 'int' is ambiguous"
//@ diag: 9:12 note "candidate function"
//@ diag: 10:12 note "candidate function"
struct L { operator int() const { return 3; } };
struct R { operator int() const { return 4; } };
struct LR : L, R {};
int amb = LR();
