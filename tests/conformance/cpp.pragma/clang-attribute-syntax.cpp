// [cpp.pragma]: the diagnostics of a malformed '#pragma clang attribute'
// (clang PragmaAttributeHandler, HandlePragmaAttribute,
// ParsePragmaAttributeSubjectMatchRuleSet, ActOnPragmaAttributeAttribute,
// ActOnPragmaAttributePop): a pragma with an error pushes nothing; a
// redundant or contradicting rule of an attribute that takes any subject is
// an error, and the attribute is still pushed; an attribute no declaration
// received is reported at its pop; a push left open at the end of the file
// is an error at the push. WP-1399.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 39:24 error "expected 'push', 'pop', or '(' after '#pragma clang attribute'"
//@ diag: 40:28 error "expected '.' after pragma attribute namespace 'foo'"
//@ diag: 41:28 error "expected '.' after pragma attribute namespace 'NS'"
//@ diag: 42:28 error "namespace can only apply to 'push' or 'pop' directives"
//@ diag: 42:28 note "omit the namespace to add attributes to the most-recently pushed attribute group"
//@ diag: 43:31 error "expected an attribute after '('"
//@ diag: 44:15 error "'#pragma clang attribute pop' with no matching '#pragma clang attribute push'"
//@ diag: 45:15 error "'#pragma clang attribute pop' with no matching '#pragma clang attribute push'"
//@ diag: 46:15 error "'#pragma clang attribute' attribute with no matching '#pragma clang attribute push'"
// (clang 22 changed two diagnostics; they are in clang-attribute-syntax-19.cpp)
//@ diag: 48:52 error "expected ','"
//@ diag: 49:53 error "expected attribute subject set specifier 'apply_to'"
//@ diag: 50:62 error "expected '='"
//@ diag: 51:65 error "unknown attribute subject rule 'fn'"
//@ diag: 52:74 error "unknown attribute subject matcher sub-rule 'is_x'; 'function' matcher supports the following sub-rules: 'is_member'"
//@ diag: 53:70 error "invalid use of attribute subject matcher sub-rule 'is_x'; 'enum' matcher does not support sub-rules"
//@ diag: 54:74 error "unknown attribute subject matcher sub-rule 'unless(is_x)'; 'variable' matcher supports the following sub-rules: 'is_thread_local', 'is_global', 'is_local', 'is_parameter', 'unless(is_parameter)'"
//@ diag: 55:79 error "duplicate attribute subject matcher 'function'"
//
//@ diag: 57:31 error "expected an attribute that is specified using the GNU, C++11 or '__declspec' syntax"
//@ diag: 57:35 note "use the GNU '__attribute__' syntax"
//@ diag: 58:72 error "expected '('"
//@ diag: 59:88 error "redundant attribute subject matcher sub-rule 'variable(is_global)'; 'variable' already matches those declarations"
//@ diag: 60:78 error "negated attribute subject matcher sub-rule 'variable(unless(is_parameter))' contradicts sub-rule 'variable(is_global)'"
//@ diag: 64:46 warning "unused attribute 'cold' in '#pragma clang attribute push' region"
//@ diag: 66:15 note "'#pragma clang attribute push' regions ends here"
//@ diag: 67:15 error "unterminated '#pragma clang attribute push' at end of file"
#pragma clang attribute
#pragma clang attribute foo
#pragma clang attribute NS push
#pragma clang attribute NS.(__attribute__((cold)), apply_to = function)
#pragma clang attribute push ()
#pragma clang attribute pop
#pragma clang attribute NS.pop
#pragma clang attribute (__attribute__((cold)), apply_to = function)
// #pragma clang attribute push with visibility: clang-attribute-syntax-19.cpp
#pragma clang attribute push (__attribute__((cold)) apply_to = function)
#pragma clang attribute push (__attribute__((cold)), to = function)
#pragma clang attribute push (__attribute__((cold)), apply_to function)
#pragma clang attribute push (__attribute__((cold)), apply_to = fn)
#pragma clang attribute push (__attribute__((cold)), apply_to = function(is_x))
#pragma clang attribute push (__attribute__((cold)), apply_to = enum(is_x))
#pragma clang attribute push (__attribute__((cold)), apply_to = variable(unless(is_x)))
#pragma clang attribute push (__attribute__((cold)), apply_to = any(function, function))
// #pragma clang attribute push with an invalid matcher: clang-attribute-syntax-19.cpp
#pragma clang attribute push (cold, apply_to = function)
#pragma clang attribute push (__attribute__((cold)), apply_to = hasType)
#pragma clang attribute push (__attribute__((annotate("a"))), apply_to = any(variable, variable(is_global)))
#pragma clang attribute push (__attribute__((annotate("a"))), apply_to = any(variable(unless(is_parameter)), variable(is_global)))
int v;
#pragma clang attribute pop
#pragma clang attribute pop
#pragma clang attribute push (__attribute__((cold)), apply_to = function)
int unused_region;
#pragma clang attribute pop
#pragma clang attribute push (__attribute__((annotate("open"))), apply_to = function)
