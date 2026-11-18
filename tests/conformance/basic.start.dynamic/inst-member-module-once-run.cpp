// Static data members and a variable template specialization with
// dynamic initializers, instantiated in module `instm` (an explicit
// instantiation of the class, of the member alone, an implicit
// instantiation, a variable template's explicit instantiation) and
// odr-used in two importers, are each initialized once: the definitions
// of the explicit instantiations are the module's, the implicit one's
// copies are comdat-keyed and test one guard ([basic.start.dynamic]/1,
// [temp.explicit]/12, [basic.def.odr]/14). They are module initializers
// (clang 22), so each importer calls `_ZGIW5instm` (WP-1669).
//@ expect: run
//@ std: c++20
//@ module: instm=Inputs/inst-member.cppm
//@ unit: Inputs/inst-member-user.cpp
//@ stdout: 7 8 9 10 34 9 calls 1111
import instm;
extern "C" int printf(const char *, ...);

int user_value();

int main() {
  printf("%d %d %d %d %d %d calls %d\n", E<int>::y, F<int>::y, G<int>::y, vt<int>, user_value(),
         g_value(), init_calls());
}
