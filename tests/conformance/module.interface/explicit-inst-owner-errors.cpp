// [temp.spec.general]/5, [temp.explicit]/11, spec/modules.md §7
// "Explicit instantiations": an explicit instantiation definition the
// imported module already holds is a duplicate in the importer, and an
// explicit instantiation declaration may not follow it, for every kind
// (clang 22 and 19 diagnose all of them; the definition stays the
// module's).
//@ expect: compile-fail
//@ std: c++20
//@ module: xiown=Inputs/explicit-inst-owner.cppm
//@ strict-diags
//@ diag: 23:15 error "duplicate explicit instantiation of 'twice<long>'"
//@ diag: 24:24 error "duplicate explicit instantiation of"
//@ diag: 25:24 error "duplicate explicit instantiation of 'v'"
//@ diag: 26:15 error "duplicate explicit instantiation of 'vt<long>'"
//@ diag: 27:17 error "duplicate explicit instantiation of 'C<long>'"
//@ diag: 28:17 error "duplicate explicit instantiation of 'D<long>'"
//@ diag: 30:22 error "explicit instantiation declaration (with 'extern') follows explicit instantiation definition"
//@ diag: 31:31 error "explicit instantiation declaration (with 'extern') follows explicit instantiation definition"
//@ diag: 32:31 error "explicit instantiation declaration (with 'extern') follows explicit instantiation definition"
//@ diag: 33:22 error "explicit instantiation declaration (with 'extern') follows explicit instantiation definition"
import xiown;

template long twice<long>(long);
template long S<long>::get(long);
template long V<long>::v;
template long vt<long>;
template struct C<long>;
template struct D<long>;

extern template long twice<long>(long);
extern template long S<long>::get(long);
extern template long V<long>::v;
extern template long vt<long>;
