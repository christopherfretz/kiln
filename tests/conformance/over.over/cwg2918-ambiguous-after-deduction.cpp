// [temp.deduct.call]/6 (CWG2918): both members give `void (*)()`, so the
// placeholder deduces; [over.over]/5 then finds neither more constrained
// (the two `!B` are different atomic constraints, [temp.constr.atomic]/2):
// ambiguous. GCC 16.1 agrees; clang 22 does not deduce from the set.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 reports the deduction as failed instead
//@ diag: 15:11 error "address of overloaded function 'g' is ambiguous"
//@ diag: 12:15 note "candidate function"
//@ diag: 13:15 note "candidate function"
template <bool B> struct X {
  static void g() requires (!B);
  static void g() requires (!B && true);
};
auto d = &X<false>::g;
