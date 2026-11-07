#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub label: String,
    pub detail: String,
    pub value: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub start: usize,
    pub end: usize,
    pub items: Vec<Suggestion>,
}

fn key(label: &str, detail: &str) -> Suggestion {
    Suggestion {
        label: label.to_owned(),
        detail: detail.to_owned(),
        value: false,
    }
}

fn value(label: &str, detail: &str) -> Suggestion {
    Suggestion {
        label: label.to_owned(),
        detail: detail.to_owned(),
        value: true,
    }
}

pub const FIXER_SETS: &[&str] = &[
    "@PSR1",
    "@PSR2",
    "@PSR12",
    "@PSR12:risky",
    "@PER-CS",
    "@PER-CS:risky",
    "@PER-CS2.0",
    "@PER-CS2.0:risky",
    "@Symfony",
    "@Symfony:risky",
    "@PhpCsFixer",
    "@PhpCsFixer:risky",
    "@DoctrineAnnotation",
    "@PHP80Migration",
    "@PHP81Migration",
    "@PHP82Migration",
    "@PHP83Migration",
    "@PHP84Migration",
    "@PHPUnit100Migration:risky",
];

pub const FIXER_RULES: &[&str] = &[
    "align_multiline_comment",
    "array_indentation",
    "array_push",
    "array_syntax",
    "assign_null_coalescing_to_coalesce_equal",
    "attribute_empty_parentheses",
    "backtick_to_shell_exec",
    "binary_operator_spaces",
    "blank_line_after_namespace",
    "blank_line_after_opening_tag",
    "blank_line_before_statement",
    "blank_line_between_import_groups",
    "blank_lines_before_namespace",
    "braces_position",
    "cast_spaces",
    "class_attributes_separation",
    "class_definition",
    "class_reference_name_casing",
    "clean_namespace",
    "combine_consecutive_issets",
    "combine_consecutive_unsets",
    "combine_nested_dirname",
    "comment_to_phpdoc",
    "compact_nullable_type_declaration",
    "concat_space",
    "constant_case",
    "control_structure_braces",
    "control_structure_continuation_position",
    "declare_equal_normalize",
    "declare_parentheses",
    "declare_strict_types",
    "dir_constant",
    "elseif",
    "empty_loop_body",
    "empty_loop_condition",
    "encoding",
    "ereg_to_preg",
    "explicit_indirect_variable",
    "explicit_string_variable",
    "final_class",
    "final_internal_class",
    "final_public_method_for_abstract_class",
    "fopen_flag_order",
    "fopen_flags",
    "fully_qualified_strict_types",
    "function_declaration",
    "function_to_constant",
    "general_phpdoc_tag_rename",
    "get_class_to_class_keyword",
    "global_namespace_import",
    "group_import",
    "heredoc_indentation",
    "heredoc_to_nowdoc",
    "implode_call",
    "include",
    "increment_style",
    "indentation_type",
    "integer_literal_case",
    "is_null",
    "lambda_not_used_import",
    "line_ending",
    "linebreak_after_opening_tag",
    "list_syntax",
    "logical_operators",
    "long_to_shorthand_operator",
    "lowercase_cast",
    "lowercase_keywords",
    "lowercase_static_reference",
    "magic_constant_casing",
    "magic_method_casing",
    "mb_str_functions",
    "method_argument_space",
    "method_chaining_indentation",
    "modernize_strpos",
    "modernize_types_casting",
    "multiline_comment_opening_closing",
    "multiline_whitespace_before_semicolons",
    "native_constant_invocation",
    "native_function_casing",
    "native_function_invocation",
    "native_type_declaration_casing",
    "new_with_parentheses",
    "no_alias_functions",
    "no_alias_language_construct_call",
    "no_alternative_syntax",
    "no_binary_string",
    "no_blank_lines_after_class_opening",
    "no_blank_lines_after_phpdoc",
    "no_break_comment",
    "no_closing_tag",
    "no_empty_comment",
    "no_empty_phpdoc",
    "no_empty_statement",
    "no_extra_blank_lines",
    "no_homoglyph_names",
    "no_leading_import_slash",
    "no_leading_namespace_whitespace",
    "no_mixed_echo_print",
    "no_multiline_whitespace_around_double_arrow",
    "no_multiple_statements_per_line",
    "no_null_property_initialization",
    "no_php4_constructor",
    "no_short_bool_cast",
    "no_singleline_whitespace_before_semicolons",
    "no_space_around_double_colon",
    "no_spaces_after_function_name",
    "no_spaces_around_offset",
    "no_superfluous_elseif",
    "no_superfluous_phpdoc_tags",
    "no_trailing_comma_in_singleline",
    "no_trailing_whitespace",
    "no_trailing_whitespace_in_comment",
    "no_trailing_whitespace_in_string",
    "no_unneeded_braces",
    "no_unneeded_control_parentheses",
    "no_unneeded_final_method",
    "no_unneeded_import_alias",
    "no_unreachable_default_argument_value",
    "no_unset_cast",
    "no_unset_on_property",
    "no_unused_imports",
    "no_useless_concat_operator",
    "no_useless_else",
    "no_useless_nullsafe_operator",
    "no_useless_return",
    "no_useless_sprintf",
    "no_whitespace_before_comma_in_array",
    "no_whitespace_in_blank_line",
    "non_printable_character",
    "normalize_index_brace",
    "not_operator_with_space",
    "not_operator_with_successor_space",
    "nullable_type_declaration",
    "nullable_type_declaration_for_default_null_value",
    "object_operator_without_whitespace",
    "octal_notation",
    "operator_linebreak",
    "ordered_class_elements",
    "ordered_imports",
    "ordered_interfaces",
    "ordered_traits",
    "ordered_types",
    "php_unit_construct",
    "php_unit_dedicate_assert",
    "php_unit_method_casing",
    "php_unit_strict",
    "php_unit_test_annotation",
    "phpdoc_add_missing_param_annotation",
    "phpdoc_align",
    "phpdoc_annotation_without_dot",
    "phpdoc_indent",
    "phpdoc_inline_tag_normalizer",
    "phpdoc_line_span",
    "phpdoc_no_access",
    "phpdoc_no_alias_tag",
    "phpdoc_no_empty_return",
    "phpdoc_no_package",
    "phpdoc_no_useless_inheritdoc",
    "phpdoc_order",
    "phpdoc_param_order",
    "phpdoc_return_self_reference",
    "phpdoc_scalar",
    "phpdoc_separation",
    "phpdoc_single_line_var_spacing",
    "phpdoc_summary",
    "phpdoc_tag_type",
    "phpdoc_to_comment",
    "phpdoc_trim",
    "phpdoc_trim_consecutive_blank_line_separation",
    "phpdoc_types",
    "phpdoc_types_order",
    "phpdoc_var_annotation_correct_order",
    "phpdoc_var_without_name",
    "pow_to_exponentiation",
    "protected_to_private",
    "psr_autoloading",
    "random_api_migration",
    "return_assignment",
    "return_type_declaration",
    "self_accessor",
    "self_static_accessor",
    "semicolon_after_instruction",
    "set_type_to_cast",
    "short_scalar_cast",
    "simple_to_complex_string_variable",
    "simplified_if_return",
    "simplified_null_return",
    "single_blank_line_at_eof",
    "single_class_element_per_statement",
    "single_import_per_statement",
    "single_line_after_imports",
    "single_line_comment_spacing",
    "single_line_comment_style",
    "single_line_empty_body",
    "single_quote",
    "single_space_around_construct",
    "space_after_semicolon",
    "spaces_inside_parentheses",
    "standardize_increment",
    "standardize_not_equals",
    "statement_indentation",
    "static_lambda",
    "strict_comparison",
    "strict_param",
    "string_implicit_backslashes",
    "string_length_to_empty",
    "switch_case_semicolon_to_colon",
    "switch_case_space",
    "switch_continue_to_break",
    "ternary_operator_spaces",
    "ternary_to_elvis_operator",
    "ternary_to_null_coalescing",
    "trailing_comma_in_multiline",
    "trim_array_spaces",
    "type_declaration_spaces",
    "types_spaces",
    "unary_operator_spaces",
    "use_arrow_functions",
    "visibility_required",
    "void_return",
    "whitespace_after_comma_in_array",
    "yoda_style",
];

fn rules() -> Vec<Suggestion> {
    let mut out: Vec<Suggestion> = FIXER_SETS.iter().map(|s| key(s, "rule set")).collect();
    out.extend(FIXER_RULES.iter().map(|r| key(r, "PHP CS Fixer rule")));
    out
}

#[derive(Debug)]
enum Frame {
    Obj {
        name: Option<String>,
        key: Option<String>,
        expect_key: bool,
    },
    Arr {
        name: Option<String>,
    },
}

fn json_context(text: &str, cursor: usize) -> Option<(usize, usize, Vec<String>, Option<String>)> {
    let b = text.as_bytes();
    let mut stack: Vec<Frame> = Vec::new();
    let mut i = 0;
    while i < b.len() && i <= cursor {
        match b[i] {
            b'"' => {
                let start = i + 1;
                let mut j = start;
                while j < b.len() && b[j] != b'"' && b[j] != b'\n' {
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                let end = j.min(b.len());
                if start <= cursor && cursor <= end {
                    let path: Vec<String> = stack
                        .iter()
                        .filter_map(|f| match f {
                            Frame::Obj { name, .. } | Frame::Arr { name } => name.clone(),
                        })
                        .collect();
                    let role = match stack.last() {
                        Some(Frame::Obj {
                            expect_key: true, ..
                        }) => None,
                        Some(Frame::Obj { key, .. }) => Some(key.clone().unwrap_or_default()),
                        Some(Frame::Arr { .. }) => Some(String::new()),
                        None => return None,
                    };
                    return Some((start, end, path, role));
                }
                let s = text[start..end].to_owned();
                if let Some(Frame::Obj {
                    key, expect_key, ..
                }) = stack.last_mut()
                    && *expect_key
                {
                    *key = Some(s);
                }
                i = end + 1;
                continue;
            }
            b'{' | b'[' => {
                let name = match stack.last() {
                    Some(Frame::Obj { key, .. }) => key.clone(),
                    Some(Frame::Arr { name }) => name.clone(),
                    None => None,
                };
                stack.push(if b[i] == b'{' {
                    Frame::Obj {
                        name,
                        key: None,
                        expect_key: true,
                    }
                } else {
                    Frame::Arr { name }
                });
            }
            b'}' | b']' => {
                stack.pop();
            }
            b':' => {
                if let Some(Frame::Obj { expect_key, .. }) = stack.last_mut() {
                    *expect_key = false;
                }
            }
            b',' => {
                if let Some(Frame::Obj {
                    expect_key, key, ..
                }) = stack.last_mut()
                {
                    *expect_key = true;
                    *key = None;
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

pub fn pint(text: &str, cursor: usize) -> Option<Completion> {
    let (start, end, path, role) = json_context(text, cursor)?;
    let items = match (path.as_slice(), role.as_deref()) {
        ([], None) => vec![
            key("preset", "laravel, per, psr12, symfony or empty"),
            key("rules", "PHP CS Fixer rules to add or override"),
            key("exclude", "folders to skip"),
            key("notName", "file name patterns to skip"),
            key("notPath", "paths to skip"),
            key("cache-file", "where Pint keeps its cache"),
        ],
        ([], Some("preset")) => ["laravel", "per", "psr12", "symfony", "empty"]
            .iter()
            .map(|p| value(p, "Pint preset"))
            .collect(),
        ([rules], None) if rules == "rules" => self::rules(),
        _ => return None,
    };
    Some(Completion { start, end, items })
}

const PHPSTAN_PARAMETERS: &[(&str, &str)] = &[
    ("level", "rule level, 0 to 10 or max"),
    ("paths", "files and folders to analyse"),
    ("excludePaths", "files and folders to skip"),
    ("bootstrapFiles", "files run before the analysis"),
    ("scanFiles", "files to read symbols from"),
    ("scanDirectories", "folders to read symbols from"),
    ("tmpDir", "cache folder"),
    ("ignoreErrors", "messages or identifiers to ignore"),
    (
        "reportUnmatchedIgnoredErrors",
        "report ignoreErrors entries that matched nothing",
    ),
    (
        "treatPhpDocTypesAsCertain",
        "trust PHPDoc types as much as native ones",
    ),
    (
        "checkMissingCallableSignature",
        "require callable signatures",
    ),
    (
        "checkUninitializedProperties",
        "report typed properties not set in the constructor",
    ),
    ("checkImplicitMixed", "treat missing types as mixed"),
    ("checkExplicitMixed", "check values typed mixed"),
    ("checkBenevolentUnionTypes", "check benevolent union types"),
    ("checkFunctionNameCase", "report wrong function name case"),
    (
        "checkInternalClassCaseSensitivity",
        "report wrong internal class name case",
    ),
    (
        "checkTooWideReturnTypesInProtectedAndPublicMethods",
        "report return types wider than needed",
    ),
    (
        "inferPrivatePropertyTypeFromConstructor",
        "infer private property types from the constructor",
    ),
    (
        "polluteScopeWithLoopInitialAssignments",
        "keep loop initial assignments after the loop",
    ),
    (
        "polluteScopeWithAlwaysIterableForeach",
        "keep foreach variables after the loop",
    ),
    (
        "rememberPossiblyImpureFunctionValues",
        "remember values of possibly impure functions",
    ),
    (
        "reportMaybesInMethodSignatures",
        "report maybe-compatible method signatures",
    ),
    (
        "reportStaticMethodSignatures",
        "check static method signature compatibility",
    ),
    (
        "reportWrongPhpDocTypeInVarTag",
        "report @var types that don't match",
    ),
    ("phpVersion", "PHP version to analyse for, like 80300"),
    ("parallel", "worker processes settings"),
    (
        "universalObjectCratesClasses",
        "classes with arbitrary properties",
    ),
    ("stubFiles", "stub files with better types"),
    ("earlyTerminatingMethodCalls", "methods that never return"),
    ("dynamicConstantNames", "constants whose value can change"),
    ("exceptions", "checked exceptions settings"),
    ("editorUrl", "link format for errors"),
    ("errorFormat", "default error formatter"),
    ("noModelMake", "Larastan: report Model::make()"),
    (
        "noUnnecessaryCollectionCall",
        "Larastan: report collection calls a query can do",
    ),
    (
        "noUnnecessaryCollectionCallOnly",
        "Larastan: methods to check for unnecessary collection calls",
    ),
    (
        "noUnnecessaryCollectionCallExcept",
        "Larastan: methods to skip for unnecessary collection calls",
    ),
    (
        "noEnvCallsOutsideOfConfig",
        "Larastan: report env() outside config files",
    ),
    (
        "checkModelProperties",
        "Larastan: check model property names in method calls",
    ),
    ("checkModelAppends", "Larastan: check $appends entries"),
    (
        "checkOctaneCompatibility",
        "Larastan: report code that breaks under Octane",
    ),
    ("checkConfigTypes", "Larastan: type config() values"),
    ("checkUnusedViews", "Larastan: report views never used"),
    (
        "checkModelMethodVisibility",
        "Larastan: report calls to non-public model methods",
    ),
    ("generalizeEnvReturnType", "Larastan: env() returns mixed"),
    (
        "databaseMigrationsPath",
        "Larastan: extra migration folders",
    ),
    ("squashedMigrationsPath", "Larastan: schema dump folders"),
    ("disableMigrationScan", "Larastan: don't read migrations"),
    ("disableSchemaScan", "Larastan: don't read schema dumps"),
];

const NEON_ROOT: &[(&str, &str)] = &[
    (
        "includes",
        "other config files, like the Larastan extension",
    ),
    ("parameters", "PHPStan and Larastan settings"),
    ("services", "extensions registered as services"),
    ("rules", "extra rule classes"),
    ("conditionalTags", "services enabled by a parameter"),
];

const NEON_INCLUDES: &[(&str, &str)] = &[
    ("vendor/larastan/larastan/extension.neon", "Larastan"),
    (
        "vendor/phpstan/phpstan-strict-rules/rules.neon",
        "strict rules",
    ),
    (
        "vendor/phpstan/phpstan-deprecation-rules/rules.neon",
        "deprecation rules",
    ),
    (
        "vendor/phpstan/phpstan/conf/bleedingEdge.neon",
        "upcoming checks",
    ),
    ("phpstan-baseline.neon", "baseline of known errors"),
];

fn neon_parent(lines: &[&str], line: usize, indent: usize) -> Option<String> {
    lines[..line].iter().rev().find_map(|l| {
        let t = l.trim_start();
        let i = l.len() - t.len();
        (!t.is_empty() && !t.starts_with('#') && i < indent).then(|| {
            t.split(':')
                .next()
                .unwrap_or("")
                .trim()
                .trim_start_matches('-')
                .trim()
                .to_owned()
        })
    })
}

pub fn neon(text: &str, cursor: usize) -> Option<Completion> {
    let line_start = text[..cursor].rfind('\n').map_or(0, |i| i + 1);
    let before = &text[line_start..cursor];
    let lines: Vec<&str> = text.lines().collect();
    let line = text[..line_start].matches('\n').count();
    let trimmed = before.trim_start();
    let indent = before.len() - trimmed.len();
    if trimmed.starts_with('#') {
        return None;
    }
    let word_start = cursor
        - trimmed
            .chars()
            .rev()
            .take_while(|c| !c.is_whitespace() && *c != ':' && *c != '-')
            .map(char::len_utf8)
            .sum::<usize>();
    if let Some((k, _)) = trimmed.split_once(':') {
        let k = k.trim().trim_start_matches('-').trim();
        let items = match k {
            "level" => (0..=10)
                .map(|n| n.to_string())
                .chain(["max".to_owned()])
                .map(|l| value(&l, "PHPStan rule level"))
                .collect(),
            "phpVersion" => ["80100", "80200", "80300", "80400", "80500"]
                .iter()
                .map(|v| value(v, "PHP version id"))
                .collect(),
            "errorFormat" => [
                "table",
                "raw",
                "json",
                "github",
                "gitlab",
                "checkstyle",
                "junit",
                "teamcity",
            ]
            .iter()
            .map(|v| value(v, "error formatter"))
            .collect(),
            k if k.starts_with("check")
                || k.starts_with("report")
                || k.starts_with("no")
                || k.starts_with("treat")
                || k.starts_with("disable")
                || k.starts_with("generalize")
                || k.starts_with("polluteScope")
                || k.starts_with("remember")
                || k.starts_with("infer") =>
            {
                vec![value("true", "boolean"), value("false", "boolean")]
            }
            _ => return None,
        };
        return Some(Completion {
            start: word_start,
            end: cursor,
            items,
        });
    }
    if trimmed.starts_with('-') {
        let parent = neon_parent(&lines, line, indent + 1)?;
        let items = match parent.as_str() {
            "includes" => NEON_INCLUDES.iter().map(|(p, d)| value(p, d)).collect(),
            "paths" | "excludePaths" | "scanDirectories" => {
                ["app", "config", "database", "routes", "tests", "bootstrap"]
                    .iter()
                    .map(|p| value(p, "project folder"))
                    .collect()
            }
            _ => return None,
        };
        return Some(Completion {
            start: word_start,
            end: cursor,
            items,
        });
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    let items: Vec<Suggestion> = if indent == 0 {
        NEON_ROOT.iter().map(|(k, d)| key(k, d)).collect()
    } else {
        match neon_parent(&lines, line, indent).as_deref() {
            Some("parameters") => PHPSTAN_PARAMETERS.iter().map(|(k, d)| key(k, d)).collect(),
            _ => return None,
        }
    };
    Some(Completion {
        start: word_start,
        end: cursor,
        items,
    })
}

pub fn cs_fixer(text: &str, cursor: usize) -> Option<Completion> {
    let rules_at = text[..cursor].rfind("setRules(")?;
    let between = &text[rules_at..cursor];
    let depth: i32 = between
        .chars()
        .map(|c| match c {
            '[' => 1,
            ']' => -1,
            _ => 0,
        })
        .sum();
    if depth != 1 {
        return None;
    }
    let quote_at = text[..cursor].rfind(['\'', '"'])?;
    if text[quote_at + 1..cursor].contains(['\n', '\'', '"']) {
        return None;
    }
    let before = text[..quote_at].trim_end();
    if !(before.ends_with('[') || before.ends_with(',')) {
        return None;
    }
    let quote = &text[quote_at..quote_at + 1];
    let end = text[cursor..]
        .find(quote)
        .filter(|e| !text[cursor..cursor + e].contains('\n'))
        .map_or(cursor, |e| cursor + e);
    Some(Completion {
        start: quote_at + 1,
        end,
        items: rules(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(c: &Option<Completion>) -> Vec<&str> {
        c.as_ref()
            .map(|c| c.items.iter().map(|i| i.label.as_str()).collect())
            .unwrap_or_default()
    }

    #[test]
    fn pint_json_keys_presets_and_rules() {
        let text = "{\n    \"pre\": \"\",\n    \"preset\": \"lar\",\n    \"rules\": {\n        \"array_\": true\n    }\n}\n";
        let keys = pint(text, text.find("pre\"").unwrap() + 3);
        assert_eq!(labels(&keys)[0], "preset");
        let c = keys.unwrap();
        assert_eq!(&text[c.start..c.end], "pre");
        assert!(labels(&pint(text, text.find("lar").unwrap() + 2)).contains(&"laravel"));
        assert!(labels(&pint(text, text.find("array_").unwrap() + 3)).contains(&"array_syntax"));
        assert!(pint(text, text.find("true").unwrap()).is_none());
    }

    #[test]
    fn neon_parameters_values_and_includes() {
        let text = "includes:\n    - vendor/\n\nparameters:\n    lev\n    level: \n    checkOctaneCompatibility: \n";
        let inc = neon(text, text.find("vendor/").unwrap() + 7);
        assert!(labels(&inc).contains(&"vendor/larastan/larastan/extension.neon"));
        assert_eq!(
            &text[inc.as_ref().unwrap().start..inc.as_ref().unwrap().end],
            "vendor/"
        );
        assert!(
            labels(&neon(text, text.find("lev\n").unwrap() + 3))
                .contains(&"noEnvCallsOutsideOfConfig")
        );
        assert!(labels(&neon(text, text.find("level: ").unwrap() + 7)).contains(&"max"));
        assert_eq!(
            labels(&neon(text, text.find("Compatibility: ").unwrap() + 15)),
            ["true", "false"]
        );
        assert!(labels(&neon("par", 3)).contains(&"parameters"));
    }

    #[test]
    fn php_cs_fixer_rule_keys() {
        let text = "<?php\nreturn (new PhpCsFixer\\Config())\n    ->setRules([\n        '@PSR12' => true,\n        'ord' => true,\n        'x' => ['syntax' => 'sh'],\n    ]);\n";
        let c = cs_fixer(text, text.find("ord").unwrap() + 3);
        assert!(labels(&c).contains(&"ordered_imports"));
        assert_eq!(
            &text[c.as_ref().unwrap().start..c.as_ref().unwrap().end],
            "ord"
        );
        assert!(cs_fixer(text, text.find("sh'").unwrap() + 1).is_none());
    }
}
