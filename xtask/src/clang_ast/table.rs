//! The clang-ast mapping table (`spec/ast-dump.md` §6): what the normalizer
//! does with every clang `-ast-dump=json` node kind. A kind that is not in
//! the table is an error naming it, so a new clang node never disappears
//! from a differential test silently.

/// What happens to a clang node of a given kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    /// Printed per `spec/ast-dump.md` §3.
    Keep,
    /// Not part of the kiln dump; the node and its subtree are skipped.
    Drop,
    /// Known, but the kiln dump has no M2 form for it yet: an error with
    /// the reason.
    Unsupported(&'static str),
}

use Disposition::{Drop, Keep, Unsupported};

const NOT_M2: &str = "not in the M2 dump (spec/ast-dump.md §3)";
const INSTANTIATION: &str =
    "template specializations and instantiations are not in the dump yet (spec/ast-dump.md §5)";
const CXX_CAST: &str = "bit casts are not in the dump";

/// Every node kind the normalizer knows, in `spec/ast-dump.md` §3 order for
/// the kept ones. Kinds ending in `Attr` and type-tree kinds under a
/// typedef are handled by rule (see [`lookup`]).
pub const TABLE: &[(&str, Disposition)] = &[
    // Declarations (§3).
    ("TranslationUnitDecl", Keep),
    ("TypedefDecl", Keep),
    ("TypeAliasDecl", Keep),
    ("CXXRecordDecl", Keep),
    ("AccessSpecDecl", Keep),
    ("FriendDecl", Keep),
    ("FieldDecl", Keep),
    ("EnumDecl", Keep),
    ("EnumConstantDecl", Keep),
    ("LinkageSpecDecl", Keep),
    // Namespaces and using-declarations (WP-062, spec/ast-dump.md §3).
    ("NamespaceDecl", Keep),
    ("NamespaceAliasDecl", Keep),
    ("UsingDecl", Keep),
    ("UsingDirectiveDecl", Keep),
    ("UsingShadowDecl", Keep),
    ("UsingEnumDecl", Keep),
    // Inheriting constructors (WP-114); clang's JSON omits the
    // nominated/constructed base classes of the text dump.
    ("ConstructorUsingShadowDecl", Keep),
    ("UnresolvedUsingValueDecl", Keep),
    ("UnresolvedUsingTypenameDecl", Keep),
    ("FunctionDecl", Keep),
    ("CXXMethodDecl", Keep),
    ("CXXConstructorDecl", Keep),
    ("CXXDestructorDecl", Keep),
    ("CXXConversionDecl", Keep),
    ("CXXCtorInitializer", Keep),
    ("ParmVarDecl", Keep),
    ("VarDecl", Keep),
    ("StaticAssertDecl", Keep),
    // GNU `__label__` local labels (WP-1288): the `DeclStmt` of a block.
    ("LabelDecl", Keep),
    // Attributes printed from the class virt-specifiers (§3).
    ("OverrideAttr", Keep),
    ("FinalAttr", Keep),
    // Expressions (§3).
    ("IntegerLiteral", Keep),
    ("FloatingLiteral", Keep),
    ("CharacterLiteral", Keep),
    ("StringLiteral", Keep),
    ("CXXBoolLiteralExpr", Keep),
    ("CXXNullPtrLiteralExpr", Keep),
    ("DeclRefExpr", Keep),
    ("MemberExpr", Keep),
    ("ImplicitCastExpr", Keep),
    ("CStyleCastExpr", Keep),
    ("CXXStaticCastExpr", Keep),
    ("CXXDynamicCastExpr", Keep),
    ("CXXConstCastExpr", Keep),
    ("CXXReinterpretCastExpr", Keep),
    ("CXXFunctionalCastExpr", Keep),
    ("UnaryOperator", Keep),
    ("BinaryOperator", Keep),
    ("CompoundAssignOperator", Keep),
    ("ConditionalOperator", Keep),
    ("ParenExpr", Keep),
    ("ArraySubscriptExpr", Keep),
    ("CallExpr", Keep),
    ("UnaryExprOrTypeTraitExpr", Keep),
    ("InitListExpr", Keep),
    ("ImplicitValueInitExpr", Keep),
    // Designated clauses of a dependent list (WP-192; template patterns).
    ("DesignatedInitExpr", Keep),
    // A designator re-initializing part of a prior initializer (WP-196).
    ("DesignatedInitUpdateExpr", Keep),
    ("NoInitExpr", Keep),
    ("ConstantExpr", Keep),
    ("CXXConstructExpr", Keep),
    ("CXXTemporaryObjectExpr", Keep),
    ("CXXInheritedCtorInitExpr", Keep),
    ("CXXMemberCallExpr", Keep),
    ("CXXThisExpr", Keep),
    ("CXXDefaultArgExpr", Keep),
    ("CXXDefaultInitExpr", Keep),
    ("CXXBindTemporaryExpr", Keep),
    ("MaterializeTemporaryExpr", Keep),
    ("CXXStdInitializerListExpr", Keep),
    ("ExprWithCleanups", Keep),
    ("CXXTypeidExpr", Keep),
    ("CXXParenListInitExpr", Keep),
    ("ArrayInitLoopExpr", Keep),
    ("ArrayInitIndexExpr", Keep),
    ("OpaqueValueExpr", Keep),
    // Expression forms (WP-064).
    ("CXXScalarValueInitExpr", Keep),
    ("CXXThrowExpr", Keep),
    ("StmtExpr", Keep),
    ("CompoundLiteralExpr", Keep),
    ("BinaryConditionalOperator", Keep),
    // The synthesized three-way comparison (WP-456).
    ("PseudoObjectExpr", Keep),
    ("OffsetOfExpr", Keep),
    // `__builtin_choose_expr` (WP-316).
    ("ChooseExpr", Keep),
    // `_Generic` (WP-1114): the controlling expression, then one `case` /
    // `default` line per association (WP-1131).
    ("GenericSelectionExpr", Keep),
    // Templates as written (WP-069, spec/ast-dump.md §5).
    ("ClassTemplateDecl", Keep),
    ("FunctionTemplateDecl", Keep),
    ("VarTemplateDecl", Keep),
    ("TypeAliasTemplateDecl", Keep),
    ("ConceptDecl", Keep),
    ("TemplateTypeParmDecl", Keep),
    ("NonTypeTemplateParmDecl", Keep),
    ("TemplateTemplateParmDecl", Keep),
    ("ConceptSpecializationExpr", Keep),
    ("RequiresExpr", Keep),
    ("SimpleRequirement", Keep),
    ("TypeRequirement", Keep),
    ("CompoundRequirement", Keep),
    ("NestedRequirement", Keep),
    ("CXXFoldExpr", Keep),
    ("CXXDependentScopeMemberExpr", Keep),
    ("CXXPseudoDestructorExpr", Keep),
    ("DependentScopeDeclRefExpr", Keep),
    ("CXXUnresolvedConstructExpr", Keep),
    ("UnresolvedLookupExpr", Keep),
    ("PackExpansionExpr", Keep),
    ("SizeOfPackExpr", Keep),
    // The concept's arguments as converted (clang-only; the written ones
    // are the `ConceptSpecializationExpr`'s own children).
    ("ImplicitConceptSpecializationDecl", Drop),
    // Statements (§3).
    ("CompoundStmt", Keep),
    ("DeclStmt", Keep),
    ("NullStmt", Keep),
    ("ReturnStmt", Keep),
    ("BreakStmt", Keep),
    ("ContinueStmt", Keep),
    ("DefaultStmt", Keep),
    ("CaseStmt", Keep),
    ("IfStmt", Keep),
    ("SwitchStmt", Keep),
    ("WhileStmt", Keep),
    ("DoStmt", Keep),
    ("ForStmt", Keep),
    ("CXXForRangeStmt", Keep),
    // Documentation comments: never part of the kiln dump.
    ("FullComment", Drop),
    ("ParagraphComment", Drop),
    ("TextComment", Drop),
    ("InlineCommandComment", Drop),
    ("BlockCommandComment", Drop),
    ("ParamCommandComment", Drop),
    ("TParamCommandComment", Drop),
    ("VerbatimBlockComment", Drop),
    ("VerbatimBlockLineComment", Drop),
    ("VerbatimLineComment", Drop),
    ("HTMLStartTagComment", Drop),
    ("HTMLEndTagComment", Drop),
    // Declarations kiln does not dump in M2.
    ("RecordDecl", Unsupported("C struct declarations (RecordDecl) are not in the M2 dump")),
    ("IndirectFieldDecl", Unsupported("anonymous struct/union members are not in the M2 dump")),
    // An implicit instantiation is printed by its `ClassTemplateDecl`; an
    // explicit one and a partial specialization at their positions.
    ("ClassTemplateSpecializationDecl", Keep),
    ("ClassTemplatePartialSpecializationDecl", Keep),
    ("VarTemplateSpecializationDecl", Keep),
    ("VarTemplatePartialSpecializationDecl", Keep),
    // User-declared guides (WP-134); implicit ones are dropped as implicit
    // decls are.
    ("CXXDeductionGuideDecl", Keep),
    // Structured bindings (WP-122, spec/ast-dump.md §3).
    ("DecompositionDecl", Keep),
    ("BindingDecl", Keep),
    ("EmptyDecl", Unsupported(NOT_M2)),
    ("FileScopeAsmDecl", Keep),
    ("ExportDecl", Unsupported(NOT_M2)),
    ("ImportDecl", Unsupported(NOT_M2)),
    ("TopLevelStmtDecl", Unsupported(NOT_M2)),
    ("LifetimeExtendedTemporaryDecl", Unsupported(NOT_M2)),
    // Statements kiln does not dump in M2.
    // Labels and `goto` (WP-160); the label is named by its `LabelStmt`.
    ("LabelStmt", Keep),
    ("GotoStmt", Keep),
    // GNU `&&label` (WP-424): the label's name.
    ("AddrLabelExpr", Keep),
    ("IndirectGotoStmt", Unsupported(NOT_M2)),
    // `[[assume]]` (WP-160), `[[likely]]`, `[[unlikely]]` and
    // `[[fallthrough]]` (WP-182) are kept on a statement; another
    // statement attribute is unsupported by its own kind.
    ("AttributedStmt", Keep),
    ("CXXAssumeAttr", Keep),
    ("LikelyAttr", Keep),
    ("UnlikelyAttr", Keep),
    ("FallThroughAttr", Keep),
    // Loop-hint pragmas (WP-449): the value, if any.
    ("LoopHintAttr", Keep),
    ("CXXTryStmt", Keep),
    ("CXXCatchStmt", Keep),
    ("GCCAsmStmt", Keep),
    ("CoreturnStmt", Unsupported(NOT_M2)),
    ("CoroutineBodyStmt", Unsupported(NOT_M2)),
    // Expressions kiln does not dump in M2.
    ("BuiltinBitCastExpr", Unsupported(CXX_CAST)),
    ("CXXOperatorCallExpr", Keep),
    ("CXXNewExpr", Keep),
    ("CXXDeleteExpr", Keep),
    ("CXXNoexceptExpr", Keep),
    ("CXXRewrittenBinaryOperator", Keep),
    ("UnresolvedMemberExpr", Unsupported("member function template names are not in the dump yet")),
    // The parameter (reprinted) then the replacement (spec/ast-dump.md §5).
    ("SubstNonTypeTemplateParmExpr", Keep),
    // A template argument outside the template nodes that print theirs
    // (an explicit specialization's or instantiation's arguments).
    ("TemplateArgument", Unsupported(INSTANTIATION)),
    // The closure (implicit, so only under `implicit`), the capture
    // initializers, then the body (spec/ast-dump.md §5).
    ("LambdaExpr", Keep),
    ("TypeTraitExpr", Unsupported(NOT_M2)),
    ("ArrayTypeTraitExpr", Unsupported(NOT_M2)),
    ("ExpressionTraitExpr", Unsupported(NOT_M2)),
    ("GNUNullExpr", Unsupported(NOT_M2)),
    ("VAArgExpr", Keep),
    ("PredefinedExpr", Keep),
    ("UserDefinedLiteral", Unsupported(NOT_M2)),
    ("ImaginaryLiteral", Unsupported(NOT_M2)),
    ("FixedPointLiteral", Unsupported(NOT_M2)),
    ("AtomicExpr", Keep),
    ("ParenListExpr", Unsupported(NOT_M2)),
    ("SourceLocExpr", Unsupported(NOT_M2)),
    ("EmbedExpr", Unsupported(NOT_M2)),
    ("CoawaitExpr", Unsupported(NOT_M2)),
    ("CoyieldExpr", Unsupported(NOT_M2)),
    ("RecoveryExpr", Unsupported("clang recovered from an error (RecoveryExpr); fix the test")),
];

/// The disposition of `kind`, or an error naming a kind the table lacks.
pub fn lookup(kind: &str) -> Result<Disposition, String> {
    if let Some((_, d)) = TABLE.iter().find(|(k, _)| *k == kind) {
        return Ok(*d);
    }
    if kind.ends_with("Attr") {
        return Ok(Unsupported("attributes are not in the M2 dump"));
    }
    Err(format!(
        "clang node kind `{kind}` is not in the clang-ast mapping table \
         (xtask/src/clang_ast/table.rs)"
    ))
}

/// JSON keys present on every node (or handled before the kind's printer).
const COMMON_KEYS: &[&str] = &["id", "kind", "loc", "range", "inner"];
/// Keys of every declaration. `TemplateInstantiationPattern` (clang 22+):
/// the id of the declaration an instantiated one was made from (class
/// template specializations and the members, nested classes, enums and
/// static data members of instantiations); ignored, kiln has no field for it
/// (WP-262, spec/harness.md "Oracle toolchain").
const DECL_KEYS: &[&str] = &[
    "isImplicit",
    "isUsed",
    "isReferenced",
    "name",
    "previousDecl",
    "parentDeclContextId",
    "mangledName",
    "TemplateInstantiationPattern",
];
/// Keys of every expression.
const EXPR_KEYS: &[&str] = &["type", "valueCategory"];

/// The node-specific JSON keys the printer of `kind` reads (or knowingly
/// ignores). Any other key is an error: it carries something the kiln line
/// would silently lose. `None` for kinds without a printer.
#[allow(
    clippy::match_same_arms,
    clippy::too_many_lines,
    reason = "a table: one arm per node kind, in spec/ast-dump.md §3 order"
)]
fn specific_keys(kind: &str) -> Option<&'static [&'static str]> {
    Some(match kind {
        "TranslationUnitDecl" => &[],
        "TypedefDecl" | "TypeAliasDecl" => &["type", "typeAliasDeclId"],
        // `definitionData` is clang's DefinitionData block (not dumped).
        "CXXRecordDecl"
        | "ClassTemplateSpecializationDecl"
        | "ClassTemplatePartialSpecializationDecl" => {
            &["tagUsed", "completeDefinition", "definitionData", "bases"]
        }
        "AccessSpecDecl" => &["access"],
        "FriendDecl" => &["type"],
        // The default member initializer is the child.
        "FieldDecl" => &["type", "isBitfield", "mutable", "hasInClassInitializer"],
        "EnumDecl" => &["scopedEnumTag", "fixedUnderlyingType"],
        "EnumConstantDecl" => &["type"],
        "LinkageSpecDecl" => &["language", "hasBraces"],
        // `isNested`: `namespace A::B` (not printed, as in clang's text dump).
        "NamespaceDecl" => &["isInline", "isNested", "originalNamespace"],
        "NamespaceAliasDecl" => &["aliasedNamespace"],
        "UsingDecl" => &[],
        "UsingDirectiveDecl" => &["nominatedNamespace"],
        "UsingShadowDecl" | "ConstructorUsingShadowDecl" => &["target"],
        "UnresolvedUsingValueDecl" | "UnresolvedUsingTypenameDecl" => &[],
        "UsingEnumDecl" => &["target"],
        // `immediate`: kiln prints clang's text-dump `consteval` instead;
        // `variadic` is part of the printed type; `deletedMessage` becomes
        // the `delete message:` child.
        "FunctionDecl" | "CXXMethodDecl" | "CXXConstructorDecl" | "CXXDestructorDecl"
        | "CXXConversionDecl" => &[
            "type",
            "storageClass",
            "inline",
            "constexpr",
            "immediate",
            "virtual",
            "pure",
            "explicitlyDefaulted",
            "explicitlyDeleted",
            "deletedMessage",
            "variadic",
        ],
        // clang's JSON has no `explicit`, and its text dump prints none.
        "CXXDeductionGuideDecl" => &["type"],
        "CXXCtorInitializer" => &["anyInit", "baseInit", "delegatingInit"],
        // `init` (always `c`, printed `cinit`): the default argument is the
        // child.
        "ParmVarDecl" => &["type", "init", "isParameterPack"],
        "VarDecl"
        | "DecompositionDecl"
        | "VarTemplateSpecializationDecl"
        | "VarTemplatePartialSpecializationDecl" => {
            &["type", "storageClass", "nrvo", "inline", "constexpr", "init"]
        }
        // The type is the binding expression's (the JSON has none).
        "BindingDecl" => &[],
        "StaticAssertDecl" | "FileScopeAsmDecl" | "LabelDecl" => &[],
        "IntegerLiteral" | "FloatingLiteral" | "CharacterLiteral" | "StringLiteral"
        | "CXXBoolLiteralExpr" => &["value"],
        "CXXNullPtrLiteralExpr"
        | "CXXNoexceptExpr"
        | "CXXPseudoDestructorExpr"
        | "AtomicExpr"
        | "VAArgExpr" => &[],
        "PredefinedExpr" => &["name"],
        // `isPlacement` and `initStyle` are implied by the children.
        "CXXNewExpr" => &[
            "isGlobal",
            "isArray",
            "isPlacement",
            "initStyle",
            "operatorNewDecl",
            "operatorDeleteDecl",
        ],
        // `isArrayAsWritten` is not printed (clang's text dump has none).
        "CXXDeleteExpr" => &["isGlobal", "isArray", "isArrayAsWritten", "operatorDeleteDecl"],
        "DeclRefExpr" => {
            &["referencedDecl", "foundReferencedDecl", "nonOdrUseReason", "isImmediateEscalating"]
        }
        "MemberExpr" => &["name", "isArrow", "referencedMemberDecl", "nonOdrUseReason"],
        // `conversionFunc` is the child's callee (not printed by clang).
        "ImplicitCastExpr" => &["castKind", "path", "isPartOfExplicitCast", "conversionFunc"],
        "CStyleCastExpr"
        | "CXXStaticCastExpr"
        | "CXXDynamicCastExpr"
        | "CXXConstCastExpr"
        | "CXXReinterpretCastExpr"
        | "CXXFunctionalCastExpr" => &["castKind", "path", "conversionFunc"],
        "UnaryOperator" => &["opcode", "isPostfix", "canOverflow"],
        "BinaryOperator" => &["opcode"],
        "CompoundAssignOperator" => &["opcode", "computeLHSType", "computeResultType"],
        "ConditionalOperator" | "ParenExpr" | "ArraySubscriptExpr" => &[],
        "CallExpr" => &["adl"],
        // The operator spelling is the callee's (clang's text dump prints it).
        "CXXOperatorCallExpr" => &["adl"],
        // Only the semantic form is a child (as in clang's text dump).
        "CXXRewrittenBinaryOperator" => &[],
        "SubstNonTypeTemplateParmExpr" => &[],
        "UnaryExprOrTypeTraitExpr" => &["name", "argType"],
        "InitListExpr" => &["field", "array_filler"],
        "ImplicitValueInitExpr" | "DesignatedInitUpdateExpr" | "NoInitExpr" => &[],
        // clang's JSON has no designators (its text dump neither).
        "DesignatedInitExpr" => &[],
        "ConstantExpr" => &["value"],
        // `hadMultipleCandidates` and `constructionKind` are not printed
        // (clang's text dump prints neither); `elidable` is rejected.
        "CXXConstructExpr" | "CXXTemporaryObjectExpr" => &[
            "ctorType",
            "list",
            "initializer_list",
            "zeroing",
            "isImmediateEscalating",
            "hadMultipleCandidates",
            "constructionKind",
        ],
        "CXXInheritedCtorInitExpr"
        | "CXXMemberCallExpr"
        | "CXXStdInitializerListExpr"
        | "CXXDefaultArgExpr"
        | "ArrayInitLoopExpr"
        | "ArrayInitIndexExpr"
        | "OpaqueValueExpr"
        | "OverrideAttr"
        | "FinalAttr"
        | "CXXScalarValueInitExpr"
        | "CXXThrowExpr"
        | "StmtExpr"
        | "CompoundLiteralExpr"
        | "BinaryConditionalOperator"
        | "PseudoObjectExpr"
        | "OffsetOfExpr"
        | "ChooseExpr" => &[],
        // The association pseudo-nodes have no `kind`; they are printed by
        // the `GenericSelectionExpr` printer.
        "GenericSelectionExpr" => &["resultDependent"],
        // `hasExplicitParameters` (clang 22+): whether the lambda spells a
        // parameter list; ignored, the parameters are the call operator's
        // (WP-262, spec/harness.md "Oracle toolchain").
        "LambdaExpr" => &["hasExplicitParameters"],
        "CXXThisExpr" => &["implicit"],
        "CXXDefaultInitExpr" => &["hasRewrittenInit"],
        // `temp` and `dtor` (the temporary's destructor) are not printed.
        "CXXBindTemporaryExpr" => &["temp", "dtor"],
        // The storage duration follows from the extending declaration.
        "MaterializeTemporaryExpr" => &["extendingDecl", "storageDuration", "boundToLValueRef"],
        "ExprWithCleanups" => &["cleanupsHaveSideEffects"],
        // `adjustedTypeArg` (the cv-unqualified type operand) is not
        // printed by clang's text dump either.
        "CXXTypeidExpr" => &["typeArg", "adjustedTypeArg"],
        "CXXParenListInitExpr" => &["array_filler"],
        "ClassTemplateDecl"
        | "FunctionTemplateDecl"
        | "VarTemplateDecl"
        | "TypeAliasTemplateDecl"
        | "ConceptDecl" => &[],
        // `defaultArg` repeats the default, which is the last child.
        "TemplateTypeParmDecl" => &["tagUsed", "depth", "index", "isParameterPack", "defaultArg"],
        "NonTypeTemplateParmDecl" => &["type", "depth", "index", "isParameterPack", "defaultArg"],
        "TemplateTemplateParmDecl" => &["depth", "index", "isParameterPack", "defaultArg"],
        // `satisfied` (a non-dependent requires-expression) prints as
        // `satisfied`/`unsatisfied`; a requirement's own is dropped.
        "RequiresExpr" => &["satisfied"],
        "SimpleRequirement" | "TypeRequirement" | "NestedRequirement" => {
            &["isDependent", "satisfied"]
        }
        "CompoundRequirement" => &["isDependent", "satisfied", "noexcept"],
        "ConceptSpecializationExpr"
        | "CXXFoldExpr"
        | "DependentScopeDeclRefExpr"
        | "PackExpansionExpr" => &[],
        // `hasTemplateKeyword` is not printed (as in clang's text dump).
        "CXXDependentScopeMemberExpr" => &[
            "isArrow",
            "member",
            "hasTemplateKeyword",
            "hasExplicitTemplateArgs",
            "explicitTemplateArgs",
        ],
        "CXXUnresolvedConstructExpr" => &["list"],
        // The found declarations are not printed (clang's text dump prints
        // their addresses).
        "UnresolvedLookupExpr" => &["usesADL", "name", "lookups"],
        "SizeOfPackExpr" => &["name"],
        "IfStmt" => {
            &["hasInit", "hasVar", "hasElse", "isConstexpr", "isConsteval", "constevalIsNegated"]
        }
        "SwitchStmt" => &["hasInit", "hasVar"],
        "WhileStmt" => &["hasVar"],
        "CompoundStmt" | "DeclStmt" | "NullStmt" | "ReturnStmt" | "BreakStmt" | "ContinueStmt"
        | "DefaultStmt" | "CaseStmt" | "DoStmt" | "ForStmt" | "CXXForRangeStmt" | "CXXTryStmt"
        | "CXXCatchStmt" | "GCCAsmStmt" | "AttributedStmt" | "CXXAssumeAttr" | "LikelyAttr"
        | "UnlikelyAttr" | "FallThroughAttr" => &[],
        "LoopHintAttr" => &["implicit"],
        "LabelStmt" => &["name", "declId"],
        "GotoStmt" => &["targetLabelDeclId"],
        "AddrLabelExpr" => &["name", "labelDeclId"],
        _ => return None,
    })
}

/// Checks that every key of a node of `kind` is one its printer accounts
/// for.
pub fn check_keys(kind: &str, keys: impl Iterator<Item = impl AsRef<str>>) -> Result<(), String> {
    let specific = specific_keys(kind)
        .ok_or_else(|| format!("clang-ast: no attribute list for kept kind `{kind}`"))?;
    let is_decl = kind.ends_with("Decl");
    let is_expr = !is_decl && !kind.ends_with("Stmt");
    for key in keys {
        let key = key.as_ref();
        let known = COMMON_KEYS.contains(&key)
            || specific.contains(&key)
            || (is_decl && DECL_KEYS.contains(&key))
            || (is_expr && EXPR_KEYS.contains(&key));
        if !known {
            return Err(format!(
                "clang-ast: `{kind}` attribute `{key}` is not in the mapping table \
                 (xtask/src/clang_ast/table.rs); the kiln dump has no field for it"
            ));
        }
    }
    Ok(())
}

/// The class kinds §3 of `spec/ast-dump.md` adds for classes (WP-066a).
#[cfg(test)]
pub const CLASS_KINDS: &[&str] = &[
    "AccessSpecDecl",
    "CXXOperatorCallExpr",
    "CXXRewrittenBinaryOperator",
    "FriendDecl",
    "CXXMethodDecl",
    "CXXConstructorDecl",
    "CXXDestructorDecl",
    "CXXConversionDecl",
    "CXXCtorInitializer",
    "OverrideAttr",
    "FinalAttr",
    "CXXStaticCastExpr",
    "CXXDynamicCastExpr",
    "CXXConstCastExpr",
    "CXXReinterpretCastExpr",
    "CXXFunctionalCastExpr",
    "CXXConstructExpr",
    "CXXTemporaryObjectExpr",
    "CXXInheritedCtorInitExpr",
    "CXXMemberCallExpr",
    "CXXThisExpr",
    "CXXDefaultArgExpr",
    "CXXDefaultInitExpr",
    "CXXBindTemporaryExpr",
    "MaterializeTemporaryExpr",
    "CXXStdInitializerListExpr",
    "ExprWithCleanups",
    "CXXTypeidExpr",
    "CXXParenListInitExpr",
    "ArrayInitLoopExpr",
    "ArrayInitIndexExpr",
    "OpaqueValueExpr",
    "DesignatedInitUpdateExpr",
    "NoInitExpr",
    // WP-075: the closure class of a lambda.
    "LambdaExpr",
    // WP-114: inheriting constructors and dependent using-declarations.
    "ConstructorUsingShadowDecl",
    "UnresolvedUsingValueDecl",
    // WP-179: `using typename B<T>::x;` in a template.
    "UnresolvedUsingTypenameDecl",
];

/// The template kinds §5 of `spec/ast-dump.md` adds (WP-069).
#[cfg(test)]
pub const TEMPLATE_KINDS: &[&str] = &[
    "ClassTemplateDecl",
    "ClassTemplateSpecializationDecl",
    "ClassTemplatePartialSpecializationDecl",
    "VarTemplateSpecializationDecl",
    "VarTemplatePartialSpecializationDecl",
    "SubstNonTypeTemplateParmExpr",
    "FunctionTemplateDecl",
    "CXXDeductionGuideDecl",
    "VarTemplateDecl",
    "TypeAliasTemplateDecl",
    "ConceptDecl",
    "TemplateTypeParmDecl",
    "NonTypeTemplateParmDecl",
    "TemplateTemplateParmDecl",
    "ConceptSpecializationExpr",
    "RequiresExpr",
    "SimpleRequirement",
    "TypeRequirement",
    "CompoundRequirement",
    "NestedRequirement",
    "CXXFoldExpr",
    "CXXDependentScopeMemberExpr",
    "DependentScopeDeclRefExpr",
    "CXXUnresolvedConstructExpr",
    "UnresolvedLookupExpr",
    "PackExpansionExpr",
    "SizeOfPackExpr",
    "DesignatedInitExpr",
];

/// The kinds §3 of `spec/ast-dump.md` defines lines for (the M2 kinds, with
/// the namespace and using-declaration kinds of WP-062).
#[cfg(test)]
pub const M2_KINDS: &[&str] = &[
    "TranslationUnitDecl",
    "TypedefDecl",
    "TypeAliasDecl",
    "CXXRecordDecl",
    "FieldDecl",
    "EnumDecl",
    "EnumConstantDecl",
    "LinkageSpecDecl",
    "NamespaceDecl",
    "NamespaceAliasDecl",
    "UsingDecl",
    "UsingDirectiveDecl",
    "UsingShadowDecl",
    "UsingEnumDecl",
    "FunctionDecl",
    "ParmVarDecl",
    "VarDecl",
    "DecompositionDecl",
    "BindingDecl",
    "StaticAssertDecl",
    // GNU local labels (WP-1288).
    "LabelDecl",
    // GNU asm (WP-140).
    "FileScopeAsmDecl",
    "GCCAsmStmt",
    // Labels, `goto`, `[[assume]]` (WP-160).
    "LabelStmt",
    "GotoStmt",
    "AddrLabelExpr",
    "AttributedStmt",
    "CXXAssumeAttr",
    // Likelihood and fallthrough statement attributes (WP-182).
    "LikelyAttr",
    "UnlikelyAttr",
    "FallThroughAttr",
    // Loop-hint pragmas (WP-449).
    "LoopHintAttr",
    // Predefined identifiers (WP-140).
    "PredefinedExpr",
    "IntegerLiteral",
    "FloatingLiteral",
    "CharacterLiteral",
    "StringLiteral",
    "CXXBoolLiteralExpr",
    "CXXNullPtrLiteralExpr",
    "DeclRefExpr",
    "MemberExpr",
    "CXXPseudoDestructorExpr",
    "ImplicitCastExpr",
    "CStyleCastExpr",
    "UnaryOperator",
    "BinaryOperator",
    "CompoundAssignOperator",
    "ConditionalOperator",
    "ParenExpr",
    "ArraySubscriptExpr",
    "CallExpr",
    "UnaryExprOrTypeTraitExpr",
    "CXXNoexceptExpr",
    "AtomicExpr",
    "VAArgExpr",
    "CXXNewExpr",
    "CXXDeleteExpr",
    "InitListExpr",
    "ImplicitValueInitExpr",
    "ConstantExpr",
    "CXXScalarValueInitExpr",
    "CXXThrowExpr",
    "StmtExpr",
    "CompoundLiteralExpr",
    "BinaryConditionalOperator",
    "PseudoObjectExpr",
    "OffsetOfExpr",
    "ChooseExpr",
    "GenericSelectionExpr",
    "CompoundStmt",
    "DeclStmt",
    "NullStmt",
    "ReturnStmt",
    "BreakStmt",
    "ContinueStmt",
    "DefaultStmt",
    "CaseStmt",
    "IfStmt",
    "SwitchStmt",
    "WhileStmt",
    "DoStmt",
    "ForStmt",
    "CXXForRangeStmt",
    "CXXTryStmt",
    "CXXCatchStmt",
];

/// Counts of table entries by disposition: (keep, drop, unsupported).
#[cfg(test)]
pub fn counts() -> (usize, usize, usize) {
    let mut c = (0, 0, 0);
    for (_, d) in TABLE {
        match d {
            Keep => c.0 += 1,
            Drop => c.1 += 1,
            Unsupported(_) => c.2 += 1,
        }
    }
    c
}
