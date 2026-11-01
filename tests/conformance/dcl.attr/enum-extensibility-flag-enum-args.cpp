// [dcl.attr.grammar]: GNU/clang enum_extensibility takes one identifier,
// open or closed; flag_enum takes none and applies to enumerations only
// (clang handleEnumExtensibilityAttr / FlagEnumAttr appertainment; WP-230)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:21 error "'enum_extensibility' attribute takes one argument"
//@ diag: 13:21 warning "'enum_extensibility' attribute argument not supported: 'foo'"
//@ diag: 14:21 error "'enum_extensibility' attribute requires parameter 0 to be an identifier"
//@ diag: 15:46 error "use of undeclared identifier 'closed'"
//@ diag: 18:21 error "'flag_enum' attribute takes no arguments"
//@ diag: 19:20 warning "'flag_enum' attribute only applies to enums"
enum __attribute__((enum_extensibility)) A { a };
enum __attribute__((enum_extensibility(foo))) B { b };
enum __attribute__((enum_extensibility(1))) C { c };
enum __attribute__((enum_extensibility(open, closed))) D { d };
enum [[clang::enum_extensibility(closed)]] E { e };
enum [[clang::flag_enum]] F { f = 1 };
enum __attribute__((flag_enum(1))) G { g = 1 };
int __attribute__((flag_enum)) x;
enum class __attribute__((flag_enum, enum_extensibility(open))) I { i = 4 };
