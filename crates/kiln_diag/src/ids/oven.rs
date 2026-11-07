// Diagnostic ids `oven_*` (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

oven_constant_evaluated_always_true = Warning [ConstantEvaluated] (1) "'{0}' will always evaluate to 'true' in a manifestly constant-evaluated expression";
oven_contract_condition_not_constant = Error (0) "contract condition is not constant";
oven_contract_condition_not_constant_observe = Warning (0) "contract condition is not constant";
oven_contract_predicate_false = Error (0) "contract predicate is false in constant expression";
oven_contract_predicate_false_observe = Warning (0) "contract predicate is false in constant expression";
oven_enum_constexpr_conversion = WarningError [EnumConstexprConversion] (4) "integer value {0} is outside the valid range of values [{1}, {2}] for the enumeration type '{3}'";
