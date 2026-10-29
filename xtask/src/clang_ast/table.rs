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
const TEMPLATES: &str = "templates are M3 (spec/ast-dump.md §5)";
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
    ("FunctionDecl", Keep),
    ("CXXMethodDecl", Keep),
    ("CXXConstructorDecl", Keep),
    ("CXXDestructorDecl", Keep),
    ("CXXConversionDecl", Keep),
    ("CXXCtorInitializer", Keep),
    ("ParmVarDecl", Keep),
    ("VarDecl", Keep),
    ("StaticAssertDecl", Keep),
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
    ("ExprWithCleanups", Keep),
    ("CXXTypeidExpr", Keep),
    ("CXXParenListInitExpr", Keep),
    ("ArrayInitLoopExpr", Keep),
    ("ArrayInitIndexExpr", Keep),
    ("OpaqueValueExpr", Keep),
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
    ("NamespaceDecl", Unsupported("namespaces are not in the M2 dump")),
    ("NamespaceAliasDecl", Unsupported(NOT_M2)),
    ("UsingDecl", Unsupported(NOT_M2)),
    ("UsingDirectiveDecl", Unsupported(NOT_M2)),
    ("UsingShadowDecl", Unsupported(NOT_M2)),
    ("UsingEnumDecl", Unsupported(NOT_M2)),
    ("IndirectFieldDecl", Unsupported("anonymous struct/union members are not in the M2 dump")),
    ("ClassTemplateDecl", Unsupported(TEMPLATES)),
    ("ClassTemplateSpecializationDecl", Unsupported(TEMPLATES)),
    ("ClassTemplatePartialSpecializationDecl", Unsupported(TEMPLATES)),
    ("FunctionTemplateDecl", Unsupported(TEMPLATES)),
    ("VarTemplateDecl", Unsupported(TEMPLATES)),
    ("VarTemplateSpecializationDecl", Unsupported(TEMPLATES)),
    ("TypeAliasTemplateDecl", Unsupported(TEMPLATES)),
    ("TemplateTypeParmDecl", Unsupported(TEMPLATES)),
    ("NonTypeTemplateParmDecl", Unsupported(TEMPLATES)),
    ("TemplateTemplateParmDecl", Unsupported(TEMPLATES)),
    ("ConceptDecl", Unsupported(TEMPLATES)),
    ("CXXDeductionGuideDecl", Unsupported(TEMPLATES)),
    ("DecompositionDecl", Unsupported("structured bindings are not in the M2 dump")),
    ("BindingDecl", Unsupported("structured bindings are not in the M2 dump")),
    ("EmptyDecl", Unsupported(NOT_M2)),
    ("LabelDecl", Unsupported(NOT_M2)),
    ("FileScopeAsmDecl", Unsupported(NOT_M2)),
    ("ExportDecl", Unsupported(NOT_M2)),
    ("ImportDecl", Unsupported(NOT_M2)),
    ("TopLevelStmtDecl", Unsupported(NOT_M2)),
    ("LifetimeExtendedTemporaryDecl", Unsupported(NOT_M2)),
    // Statements kiln does not dump in M2.
    ("LabelStmt", Unsupported(NOT_M2)),
    ("GotoStmt", Unsupported(NOT_M2)),
    ("IndirectGotoStmt", Unsupported(NOT_M2)),
    ("AttributedStmt", Unsupported("attributes are not in the M2 dump")),
    ("CXXForRangeStmt", Unsupported(NOT_M2)),
    ("CXXTryStmt", Unsupported(NOT_M2)),
    ("CXXCatchStmt", Unsupported(NOT_M2)),
    ("GCCAsmStmt", Unsupported(NOT_M2)),
    ("CoreturnStmt", Unsupported(NOT_M2)),
    ("CoroutineBodyStmt", Unsupported(NOT_M2)),
    // Expressions kiln does not dump in M2.
    ("BuiltinBitCastExpr", Unsupported(CXX_CAST)),
    ("CXXOperatorCallExpr", Unsupported("overloaded operator calls are not in the dump yet")),
    ("CXXNewExpr", Unsupported(NOT_M2)),
    ("CXXDeleteExpr", Unsupported(NOT_M2)),
    ("CXXThrowExpr", Unsupported(NOT_M2)),
    ("CXXNoexceptExpr", Keep),
    ("CXXScalarValueInitExpr", Unsupported(NOT_M2)),
    ("CXXStdInitializerListExpr", Unsupported(NOT_M2)),
    ("CXXRewrittenBinaryOperator", Unsupported(NOT_M2)),
    ("CXXFoldExpr", Unsupported(TEMPLATES)),
    ("CXXDependentScopeMemberExpr", Unsupported(TEMPLATES)),
    ("CXXUnresolvedConstructExpr", Unsupported(TEMPLATES)),
    ("UnresolvedLookupExpr", Unsupported(TEMPLATES)),
    ("UnresolvedMemberExpr", Unsupported(TEMPLATES)),
    ("DependentScopeDeclRefExpr", Unsupported(TEMPLATES)),
    ("PackExpansionExpr", Unsupported(TEMPLATES)),
    ("SizeOfPackExpr", Unsupported(TEMPLATES)),
    ("SubstNonTypeTemplateParmExpr", Unsupported(TEMPLATES)),
    ("ConceptSpecializationExpr", Unsupported(TEMPLATES)),
    ("RequiresExpr", Unsupported(TEMPLATES)),
    ("LambdaExpr", Unsupported("lambdas are M3 (spec/ast-dump.md §5)")),
    ("TypeTraitExpr", Unsupported(NOT_M2)),
    ("ArrayTypeTraitExpr", Unsupported(NOT_M2)),
    ("ExpressionTraitExpr", Unsupported(NOT_M2)),
    ("StmtExpr", Unsupported(NOT_M2)),
    ("ChooseExpr", Unsupported(NOT_M2)),
    ("GNUNullExpr", Unsupported(NOT_M2)),
    ("VAArgExpr", Unsupported(NOT_M2)),
    ("PredefinedExpr", Unsupported(NOT_M2)),
    ("UserDefinedLiteral", Unsupported(NOT_M2)),
    ("CompoundLiteralExpr", Unsupported(NOT_M2)),
    ("DesignatedInitExpr", Unsupported(NOT_M2)),
    ("DesignatedInitUpdateExpr", Unsupported(NOT_M2)),
    ("NoInitExpr", Unsupported(NOT_M2)),
    ("BinaryConditionalOperator", Unsupported(NOT_M2)),
    ("ImaginaryLiteral", Unsupported(NOT_M2)),
    ("FixedPointLiteral", Unsupported(NOT_M2)),
    ("AtomicExpr", Unsupported(NOT_M2)),
    ("ParenListExpr", Unsupported(NOT_M2)),
    ("SourceLocExpr", Unsupported(NOT_M2)),
    ("EmbedExpr", Unsupported(NOT_M2)),
    ("OffsetOfExpr", Unsupported(NOT_M2)),
    ("AddrLabelExpr", Unsupported(NOT_M2)),
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
/// Keys of every declaration.
const DECL_KEYS: &[&str] = &[
    "isImplicit",
    "isUsed",
    "isReferenced",
    "name",
    "previousDecl",
    "parentDeclContextId",
    "mangledName",
];
/// Keys of every expression.
const EXPR_KEYS: &[&str] = &["type", "valueCategory"];

/// The node-specific JSON keys the printer of `kind` reads (or knowingly
/// ignores). Any other key is an error: it carries something the kiln line
/// would silently lose. `None` for kinds without a printer.
#[allow(clippy::match_same_arms, reason = "one arm per node kind, in spec/ast-dump.md §3 order")]
fn specific_keys(kind: &str) -> Option<&'static [&'static str]> {
    Some(match kind {
        "TranslationUnitDecl" => &[],
        "TypedefDecl" | "TypeAliasDecl" => &["type", "typeAliasDeclId"],
        // `definitionData` is clang's DefinitionData block (not dumped).
        "CXXRecordDecl" => &["tagUsed", "completeDefinition", "definitionData", "bases"],
        "AccessSpecDecl" => &["access"],
        "FriendDecl" => &["type"],
        // The default member initializer is the child.
        "FieldDecl" => &["type", "isBitfield", "mutable", "hasInClassInitializer"],
        "EnumDecl" => &["scopedEnumTag", "fixedUnderlyingType"],
        "EnumConstantDecl" => &["type"],
        "LinkageSpecDecl" => &["language", "hasBraces"],
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
        "CXXCtorInitializer" => &["anyInit", "baseInit", "delegatingInit"],
        // `init` (always `c`): the default argument is the child.
        "ParmVarDecl" => &["type", "init"],
        "VarDecl" => &["type", "storageClass", "nrvo", "inline", "constexpr", "init"],
        "StaticAssertDecl" => &[],
        "IntegerLiteral" | "FloatingLiteral" | "CharacterLiteral" | "StringLiteral"
        | "CXXBoolLiteralExpr" => &["value"],
        "CXXNullPtrLiteralExpr" | "CXXNoexceptExpr" => &[],
        "DeclRefExpr" => &["referencedDecl", "nonOdrUseReason"],
        "MemberExpr" => &["name", "isArrow", "referencedMemberDecl"],
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
        "ConditionalOperator" | "ParenExpr" | "ArraySubscriptExpr" | "CallExpr" => &[],
        "UnaryExprOrTypeTraitExpr" => &["name", "argType"],
        "InitListExpr" => &["field", "array_filler"],
        "ImplicitValueInitExpr" => &[],
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
        | "CXXDefaultArgExpr"
        | "ArrayInitLoopExpr"
        | "ArrayInitIndexExpr"
        | "OpaqueValueExpr"
        | "OverrideAttr"
        | "FinalAttr" => &[],
        "CXXThisExpr" => &["implicit"],
        "CXXDefaultInitExpr" => &["hasRewrittenInit"],
        // `temp` and `dtor` (the temporary's destructor) are not printed.
        "CXXBindTemporaryExpr" => &["temp", "dtor"],
        // The storage duration follows from the extending declaration.
        "MaterializeTemporaryExpr" => &["extendingDecl", "storageDuration", "boundToLValueRef"],
        "ExprWithCleanups" => &["cleanupsHaveSideEffects"],
        "CXXTypeidExpr" => &["typeArg"],
        "CXXParenListInitExpr" => &["array_filler"],
        "IfStmt" => {
            &["hasInit", "hasVar", "hasElse", "isConstexpr", "isConsteval", "constevalIsNegated"]
        }
        "SwitchStmt" => &["hasInit", "hasVar"],
        "WhileStmt" => &["hasVar"],
        "CompoundStmt" | "DeclStmt" | "NullStmt" | "ReturnStmt" | "BreakStmt" | "ContinueStmt"
        | "DefaultStmt" | "CaseStmt" | "DoStmt" | "ForStmt" => &[],
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
    "ExprWithCleanups",
    "CXXTypeidExpr",
    "CXXParenListInitExpr",
    "ArrayInitLoopExpr",
    "ArrayInitIndexExpr",
    "OpaqueValueExpr",
];

/// The kinds §3 of `spec/ast-dump.md` defines lines for (the M2 kinds).
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
    "FunctionDecl",
    "ParmVarDecl",
    "VarDecl",
    "StaticAssertDecl",
    "IntegerLiteral",
    "FloatingLiteral",
    "CharacterLiteral",
    "StringLiteral",
    "CXXBoolLiteralExpr",
    "CXXNullPtrLiteralExpr",
    "DeclRefExpr",
    "MemberExpr",
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
    "InitListExpr",
    "ImplicitValueInitExpr",
    "ConstantExpr",
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
