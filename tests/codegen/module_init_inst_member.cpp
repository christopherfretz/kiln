// Static data members and a variable template specialization with
// dynamic initializers instantiated in a module interface (an explicit
// instantiation of the class, of the member, an implicit instantiation,
// a variable template's explicit instantiation) are module initializers
// for clang 22: an importer calls `_ZGIW` though they are the module's
// only ones (WP-1669).
//@ module: mod_init_inst=Inputs/mod_init_inst_member.cppm
import mod_init_inst;
int main() { return E<int>::y + F<int>::y + G<int>::y + vt<int>; }
