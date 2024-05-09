// Copyright 2021-2024 The Jujutsu Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(missing_docs)]

use std::collections::HashSet;
use std::error;

use pest_derive::Parser;
use thiserror::Error;

use crate::dsl_util::StringLiteralParser;

// TODO: remove pub(super)
#[derive(Parser)]
#[grammar = "revset.pest"]
pub(super) struct RevsetParser;

// TODO: remove pub(super)
pub(super) const STRING_LITERAL_PARSER: StringLiteralParser<Rule> = StringLiteralParser {
    content_rule: Rule::string_content,
    escape_rule: Rule::string_escape,
};

impl Rule {
    /// Whether this is a placeholder rule for compatibility with the other
    /// systems.
    fn is_compat(&self) -> bool {
        matches!(
            self,
            Rule::compat_parents_op
                | Rule::compat_dag_range_op
                | Rule::compat_dag_range_pre_op
                | Rule::compat_dag_range_post_op
                | Rule::compat_add_op
                | Rule::compat_sub_op
        )
    }

    fn to_symbol(self) -> Option<&'static str> {
        match self {
            Rule::EOI => None,
            Rule::whitespace => None,
            Rule::identifier_part => None,
            Rule::identifier => None,
            Rule::symbol => None,
            Rule::string_escape => None,
            Rule::string_content_char => None,
            Rule::string_content => None,
            Rule::string_literal => None,
            Rule::raw_string_content => None,
            Rule::raw_string_literal => None,
            Rule::at_op => Some("@"),
            Rule::pattern_kind_op => Some(":"),
            Rule::parents_op => Some("-"),
            Rule::children_op => Some("+"),
            Rule::compat_parents_op => Some("^"),
            Rule::dag_range_op
            | Rule::dag_range_pre_op
            | Rule::dag_range_post_op
            | Rule::dag_range_all_op => Some("::"),
            Rule::compat_dag_range_op
            | Rule::compat_dag_range_pre_op
            | Rule::compat_dag_range_post_op => Some(":"),
            Rule::range_op => Some(".."),
            Rule::range_pre_op | Rule::range_post_op | Rule::range_all_op => Some(".."),
            Rule::range_ops => None,
            Rule::range_pre_ops => None,
            Rule::range_post_ops => None,
            Rule::range_all_ops => None,
            Rule::negate_op => Some("~"),
            Rule::union_op => Some("|"),
            Rule::intersection_op => Some("&"),
            Rule::difference_op => Some("~"),
            Rule::compat_add_op => Some("+"),
            Rule::compat_sub_op => Some("-"),
            Rule::infix_op => None,
            Rule::function_name => None,
            Rule::keyword_argument => None,
            Rule::argument => None,
            Rule::function_arguments => None,
            Rule::formal_parameters => None,
            Rule::string_pattern => None,
            Rule::primary => None,
            Rule::neighbors_expression => None,
            Rule::range_expression => None,
            Rule::expression => None,
            Rule::program => None,
            Rule::program_modifier => None,
            Rule::program_with_modifier => None,
            Rule::alias_declaration_part => None,
            Rule::alias_declaration => None,
        }
    }
}

#[derive(Debug, Error)]
#[error("{pest_error}")]
pub struct RevsetParseError {
    // TODO: move parsing tests to this module and drop pub(super)
    pub(super) kind: RevsetParseErrorKind,
    pest_error: Box<pest::error::Error<Rule>>,
    source: Option<Box<dyn error::Error + Send + Sync>>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RevsetParseErrorKind {
    #[error("Syntax error")]
    SyntaxError,
    #[error("'{op}' is not a prefix operator")]
    NotPrefixOperator {
        op: String,
        similar_op: String,
        description: String,
    },
    #[error("'{op}' is not a postfix operator")]
    NotPostfixOperator {
        op: String,
        similar_op: String,
        description: String,
    },
    #[error("'{op}' is not an infix operator")]
    NotInfixOperator {
        op: String,
        similar_op: String,
        description: String,
    },
    #[error(r#"Modifier "{0}" doesn't exist"#)]
    NoSuchModifier(String),
    #[error(r#"Function "{name}" doesn't exist"#)]
    NoSuchFunction {
        name: String,
        candidates: Vec<String>,
    },
    #[error(r#"Function "{name}": {message}"#)]
    InvalidFunctionArguments { name: String, message: String },
    #[error("Cannot resolve file pattern without workspace")]
    FsPathWithoutWorkspace,
    #[error(r#"Cannot resolve "@" without workspace"#)]
    WorkingCopyWithoutWorkspace,
    #[error("Redefinition of function parameter")]
    RedefinedFunctionParameter,
    #[error(r#"Alias "{0}" cannot be expanded"#)]
    BadAliasExpansion(String),
    #[error(r#"Alias "{0}" expanded recursively"#)]
    RecursiveAlias(String),
}

impl RevsetParseError {
    pub(super) fn with_span(kind: RevsetParseErrorKind, span: pest::Span<'_>) -> Self {
        let message = kind.to_string();
        let pest_error = Box::new(pest::error::Error::new_from_span(
            pest::error::ErrorVariant::CustomError { message },
            span,
        ));
        RevsetParseError {
            kind,
            pest_error,
            source: None,
        }
    }

    pub(super) fn with_source(
        mut self,
        source: impl Into<Box<dyn error::Error + Send + Sync>>,
    ) -> Self {
        self.source = Some(source.into());
        self
    }

    pub(super) fn invalid_arguments(
        name: impl Into<String>,
        message: impl Into<String>,
        span: pest::Span<'_>,
    ) -> Self {
        Self::with_span(
            RevsetParseErrorKind::InvalidFunctionArguments {
                name: name.into(),
                message: message.into(),
            },
            span,
        )
    }

    pub fn kind(&self) -> &RevsetParseErrorKind {
        &self.kind
    }

    /// Original parsing error which typically occurred in an alias expression.
    pub fn origin(&self) -> Option<&Self> {
        self.source.as_ref().and_then(|e| e.downcast_ref())
    }
}

impl From<pest::error::Error<Rule>> for RevsetParseError {
    fn from(err: pest::error::Error<Rule>) -> Self {
        RevsetParseError {
            kind: RevsetParseErrorKind::SyntaxError,
            pest_error: Box::new(rename_rules_in_pest_error(err)),
            source: None,
        }
    }
}

fn rename_rules_in_pest_error(mut err: pest::error::Error<Rule>) -> pest::error::Error<Rule> {
    let pest::error::ErrorVariant::ParsingError {
        positives,
        negatives,
    } = &mut err.variant
    else {
        return err;
    };

    // Remove duplicated symbols. Compat symbols are also removed from the
    // (positive) suggestion.
    let mut known_syms = HashSet::new();
    positives.retain(|rule| {
        !rule.is_compat() && rule.to_symbol().map_or(true, |sym| known_syms.insert(sym))
    });
    let mut known_syms = HashSet::new();
    negatives.retain(|rule| rule.to_symbol().map_or(true, |sym| known_syms.insert(sym)));
    err.renamed_rules(|rule| {
        rule.to_symbol()
            .map(|sym| format!("`{sym}`"))
            .unwrap_or_else(|| format!("<{rule:?}>"))
    })
}