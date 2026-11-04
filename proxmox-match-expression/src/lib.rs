//! De/serializable expression evaluation with custom leaf-nodes.
//!
//! ## Example:
//!```
//! use serde::{Serialize, Deserialize};
//! use pretty_assertions::{assert_eq, assert_str_eq};
//!
//! use proxmox_match_expression::{any_of, Expression, MatchExpression};
//!
//! #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
//! #[serde(rename_all = "kebab-case", tag = "type")]
//! enum StringMatcher {
//!     AllUppercase,
//!     Content { value: String },
//! }
//!
//! impl MatchExpression for StringMatcher {
//!     type Data = String;
//!     type Error = ();
//!
//!    fn evaluate(&self, data: &Self::Data) -> Result<bool, ()> {
//!         match self {
//!             StringMatcher::AllUppercase => Ok(&data.to_uppercase() == data),
//!             StringMatcher::Content { value } => Ok(value == data),
//!         }
//!     }
//! }
//!
//! impl From<StringMatcher> for Expression<StringMatcher> {
//!     fn from(matcher: StringMatcher) -> Self {
//!         Expression::Match(matcher)
//!     }
//! }
//!
//! // You can use macros to conveniently construct expressions in code.
//! let expr = any_of![
//!     StringMatcher::Content {
//!         value: "foo".to_string(),
//!     }
//!     .into(),
//!     StringMatcher::AllUppercase.into(),
//! ];
//!
//! // Expressions implement `Serialize` and `Deserialize`.
//! let expected_serialized_expr = r#"{
//!  "any-of": [
//!    {
//!      "match": {
//!        "type": "content",
//!        "value": "foo"
//!      }
//!    },
//!    {
//!      "match": {
//!        "type": "all-uppercase"
//!      }
//!    }
//!  ]
//!}"#;
//!
//! assert_eq!(serde_json::to_string_pretty(&expr).unwrap(), expected_serialized_expr);
//!
//! assert!(expr.evaluate(&"foo".to_string()).unwrap().is_match());
//! assert!(!expr.evaluate(&"fo".to_string()).unwrap().is_match());
//! assert!(!expr.evaluate(&"fooo".to_string()).unwrap().is_match());
//!
//! let evaluated_expression = expr.evaluate(&"AAAAA".to_string()).unwrap();
//! assert!(evaluated_expression.is_match());
//!
//! // `Expression::evaluate` returns an `EvaluatedExpressionWithResult`, which contains a full
//! // trace of all expressions that were evaluated.
//!
//! let serialized = serde_json::to_string_pretty(&evaluated_expression).unwrap();
//! let expected = r#"{
//!  "any-of": [
//!    {
//!      "match": {
//!        "type": "content",
//!        "value": "foo"
//!      },
//!      "matches": false
//!    },
//!    {
//!      "match": {
//!        "type": "all-uppercase"
//!      },
//!      "matches": true
//!    }
//!  ],
//!  "matches": true
//!}"#;
//!
//! assert_str_eq!(serialized, expected);
//!
//! assert!(!evaluated_expression.children()[0].is_match());
//! assert!(evaluated_expression.children()[1].is_match());
//!
//! assert_eq!(evaluated_expression.children()[1].children(), &[]);
//!
//! ```

use serde::{Deserialize, Serialize};

/// Trait for custom expression leaf-nodes.
pub trait MatchExpression {
    /// Use this type to specify the type of the data the expression is evaluated against.
    type Data;

    /// Error type returned when evaluating this custom expression.
    type Error;

    /// Evaluate this expression.
    ///
    /// If there are no errors, this should return `Ok(true)` or `Ok(false)`, depending on whether
    /// this leaf-node expression matched the provided data or not.
    fn evaluate(&self, data: &Self::Data) -> Result<bool, Self::Error>;
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
/// Match expression.
pub enum Expression<M> {
    /// Match if all of the sub-expressions match.
    AllOf(Vec<Expression<M>>),
    /// Match if any of the sub-expressions matches.
    AnyOf(Vec<Expression<M>>),
    /// Match if exactly one of the sub-expressions matches.
    OneOf(Vec<Expression<M>>),
    /// Match if the sub-expression does not match.
    Not(Box<Expression<M>>),
    /// Constant value (true or false).
    Constant(bool),
    /// Custom matcher leaf-nodes.
    Match(M),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
/// Evaluated match expression.
pub enum EvaluatedExpression<M> {
    /// Match if all of the sub-expressions match.
    AllOf(Vec<EvaluatedExpressionWithResult<M>>),
    /// Match if any of the sub-expressions matches.
    AnyOf(Vec<EvaluatedExpressionWithResult<M>>),
    /// Match if exactly one of the sub-expressions matches.
    OneOf(Vec<EvaluatedExpressionWithResult<M>>),
    /// Match if the sub-expression does not match.
    Not(Box<EvaluatedExpressionWithResult<M>>),
    /// Constant value (true or false).
    Constant(bool),
    /// Custom matcher leaf-nodes.
    Match(M),
}

/// Evaluated expression resulting from [`Expression::evaluate`].
///
/// This type contains the original expression, augmented with per-node results. This is useful for
/// recording 'traces', e.g. for recording exactly why an expression matched on the data or not.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedExpressionWithResult<M> {
    /// The actual expression.
    #[serde(flatten)]
    expression: EvaluatedExpression<M>,
    /// The evaluated expression matched the provided data.
    matches: bool,
}

impl<M> EvaluatedExpressionWithResult<M> {
    /// Return whether the expression matched the provided data.
    pub fn is_match(&self) -> bool {
        self.matches
    }

    /// Borrow the contained [`EvaluatedExpression`].
    pub fn expression(&self) -> &EvaluatedExpression<M> {
        &self.expression
    }

    /// Return the contained [`EvaluatedExpression`], consuming `self`.
    pub fn into_expression(self) -> EvaluatedExpression<M> {
        self.expression
    }

    /// The direct child nodes, uniform across all combinator kinds.
    ///
    /// 'not' yields its single operand as a one-element slice; leaf nodes ('constant', 'match')
    /// yield an empty slice. This lets a generic tree walker recurse without special-casing each
    /// variant.
    pub fn children(&self) -> &[EvaluatedExpressionWithResult<M>] {
        match &self.expression {
            EvaluatedExpression::AllOf(expressions)
            | EvaluatedExpression::AnyOf(expressions)
            | EvaluatedExpression::OneOf(expressions) => expressions,
            EvaluatedExpression::Not(expression) => std::slice::from_ref(&**expression),
            EvaluatedExpression::Constant(_) | EvaluatedExpression::Match(_) => &[],
        }
    }
}

impl<M> From<EvaluatedExpressionWithResult<M>> for EvaluatedExpression<M> {
    fn from(value: EvaluatedExpressionWithResult<M>) -> Self {
        value.into_expression()
    }
}

impl<M> Expression<M> {
    /// The direct child nodes, uniform across all combinator kinds.
    ///
    /// 'not' yields its single operand as a one-element slice; leaf nodes ('constant', 'match')
    /// yield an empty slice. This lets a generic tree walker recurse without special-casing each
    /// variant.
    pub fn children(&self) -> &[Expression<M>] {
        match self {
            Expression::AllOf(expressions)
            | Expression::AnyOf(expressions)
            | Expression::OneOf(expressions) => expressions,
            Expression::Not(expression) => std::slice::from_ref(&**expression),
            Expression::Constant(_) | Expression::Match(_) => &[],
        }
    }
}

impl<M> Expression<M>
where
    M: Clone + MatchExpression,
{
    /// Evaluate this expression against the provided data.
    ///
    /// The returned [`EvaluatedExpressionWithResult`] replicates the structure of `self`, annotated
    /// with whether each sub-expression matched.
    ///
    /// Evaluation does not short-circuit when the boolean result is determined, for instance by a
    /// matching first sub-expression of [`Expression::AnyOf`], so that on success the trace
    /// contains every sub-expression.
    ///
    /// # Errors
    ///
    /// Returns immediately on the first error from a [`MatchExpression`] leaf-node. The whole
    /// evaluation fails without returning a trace, even if the result does not depend on that
    /// leaf-node.
    pub fn evaluate(&self, data: &M::Data) -> Result<EvaluatedExpressionWithResult<M>, M::Error> {
        let evaluated_expression = match self {
            Expression::AllOf(expressions) => {
                let (matches, evaluated_expressions) =
                    Self::evaluate_subexpressions(data, expressions, true, |a, b| a && b)?;

                EvaluatedExpressionWithResult {
                    expression: EvaluatedExpression::AllOf(evaluated_expressions),
                    matches,
                }
            }
            Expression::AnyOf(expressions) => {
                let (matches, evaluated_expressions) =
                    Self::evaluate_subexpressions(data, expressions, false, |a, b| a || b)?;

                EvaluatedExpressionWithResult {
                    expression: EvaluatedExpression::AnyOf(evaluated_expressions),
                    matches,
                }
            }
            Expression::OneOf(expressions) => {
                let mut matched_count = 0;
                let mut evaluated_expressions = Vec::new();

                for expression in expressions {
                    let evaluated_expression = expression.evaluate(data)?;
                    if evaluated_expression.is_match() {
                        matched_count += 1;
                    }
                    evaluated_expressions.push(evaluated_expression)
                }

                EvaluatedExpressionWithResult {
                    expression: EvaluatedExpression::OneOf(evaluated_expressions),
                    matches: matched_count == 1,
                }
            }
            Expression::Not(expression) => {
                let evaluated_expression = expression.evaluate(data)?;
                let matches = !evaluated_expression.is_match();
                EvaluatedExpressionWithResult {
                    expression: EvaluatedExpression::Not(Box::new(evaluated_expression)),
                    matches,
                }
            }
            Expression::Constant(value) => EvaluatedExpressionWithResult {
                expression: EvaluatedExpression::Constant(*value),
                matches: *value,
            },
            Expression::Match(inner) => {
                let matches = inner.evaluate(data)?;
                EvaluatedExpressionWithResult {
                    expression: EvaluatedExpression::Match(inner.clone()),
                    matches,
                }
            }
        };

        Ok(evaluated_expression)
    }

    fn evaluate_subexpressions(
        data: &M::Data,
        expressions: &[Expression<M>],
        initial: bool,
        op: impl Fn(bool, bool) -> bool,
    ) -> Result<(bool, Vec<EvaluatedExpressionWithResult<M>>), M::Error> {
        let mut matches = initial;

        let mut evaluated_expressions = Vec::new();

        for expression in expressions {
            let evaluated_expression = expression.evaluate(data)?;
            matches = op(matches, evaluated_expression.is_match());
            evaluated_expressions.push(evaluated_expression);
        }

        Ok((matches, evaluated_expressions))
    }
}

/// Convenience macro that can be used to create an 'AnyOf' expression.
///
/// ```
/// use proxmox_match_expression::{any_of, Expression};
///
/// let x: Expression<()> = any_of![
///     Expression::Constant(true),
/// ];
/// assert_eq!(x, Expression::AnyOf(vec![Expression::Constant(true)]));
/// ```
///
#[macro_export]
macro_rules! any_of {
    ($($x:expr),* $(,)?) => (
        $crate::Expression::AnyOf(vec![$($x),*])
    );
}

/// Convenience macro that can be used to create an 'AllOf' expression.
///
/// ```
/// use proxmox_match_expression::{all_of, Expression};
///
/// let x: Expression<()> = all_of![
///     Expression::Constant(true),
/// ];
/// assert_eq!(x, Expression::AllOf(vec![Expression::Constant(true)]));
/// ```
#[macro_export]
macro_rules! all_of {
    ($($x:expr),* $(,)?) => (
        $crate::Expression::AllOf(vec![$($x),*])
    );
}

/// Convenience macro that can be used to create an 'OneOf' expression.
///
/// ```
/// use proxmox_match_expression::{one_of, Expression};
///
/// let x: Expression<()> = one_of![
///     Expression::Constant(true),
/// ];
/// assert_eq!(x, Expression::OneOf(vec![Expression::Constant(true)]));
/// ```
#[macro_export]
macro_rules! one_of {
    ($($x:expr),* $(,)?) => (
        $crate::Expression::OneOf(vec![$($x),*])
    );
}

/// Convenience macro that can be used to create a 'Not' expression;
///
/// ```
/// use proxmox_match_expression::{not, Expression};
///
/// let x: Expression<()> = not!(Expression::Constant(true));
/// assert_eq!(x, Expression::Not(Box::new(Expression::Constant(true))));
/// ```
#[macro_export]
macro_rules! not {
    ($e:expr) => {
        $crate::Expression::Not(Box::new($e))
    };
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(rename_all = "kebab-case", tag = "type")]
    pub enum TestMatcher {
        Failure,
        True,
        False,
    }

    impl MatchExpression for TestMatcher {
        type Data = ();
        type Error = ();

        fn evaluate(&self, _data: &Self::Data) -> Result<bool, ()> {
            match self {
                TestMatcher::Failure => Err(()),
                TestMatcher::True => Ok(true),
                TestMatcher::False => Ok(false),
            }
        }
    }

    fn eval(expr: Expression<TestMatcher>) -> Result<bool, ()> {
        let e = expr.evaluate(&())?;

        Ok(e.is_match())
    }

    #[test]
    fn test_all_of() {
        let expr = all_of![Expression::Constant(true), Expression::Constant(false)];
        assert!(!eval(expr).unwrap());

        let expr = all_of![Expression::Constant(true), Expression::Constant(true)];
        assert!(eval(expr).unwrap());

        let expr = all_of![Expression::Constant(true)];
        assert!(eval(expr).unwrap());

        let expr: Expression<TestMatcher> = all_of![];
        assert!(eval(expr).unwrap());

        let expr = all_of![Expression::Match(TestMatcher::Failure)];
        assert!(eval(expr).is_err());
    }

    #[test]
    fn test_any_of() {
        let expr = any_of![Expression::Constant(true), Expression::Constant(false)];
        assert!(eval(expr).unwrap());

        let expr = any_of![Expression::Constant(false), Expression::Constant(false)];
        assert!(!eval(expr).unwrap());

        let expr = any_of![Expression::Constant(true)];
        assert!(eval(expr).unwrap());

        let expr: Expression<TestMatcher> = any_of![];
        assert!(!eval(expr).unwrap());

        let expr = any_of![Expression::Match(TestMatcher::Failure)];
        assert!(eval(expr).is_err());
    }

    #[test]
    fn test_one_of() {
        let expr = one_of![Expression::Constant(true), Expression::Constant(false)];
        assert!(eval(expr).unwrap());

        let expr = one_of![Expression::Constant(false)];
        assert!(!eval(expr).unwrap());

        let expr = one_of![Expression::Constant(true)];
        assert!(eval(expr).unwrap());

        let expr: Expression<TestMatcher> = one_of![];
        assert!(!eval(expr).unwrap());

        let expr = one_of![Expression::Match(TestMatcher::Failure)];
        assert!(eval(expr).is_err());

        // Exactly one of three sub-expressions matches.
        let expr = one_of![
            Expression::Constant(false),
            Expression::Constant(true),
            Expression::Constant(false),
        ];
        assert!(eval(expr).unwrap());

        // Two of three match. This must *not* match: it distinguishes 'exactly one' from
        // XOR-parity, which would report a match for any odd number of matching sub-expressions.
        let expr = one_of![
            Expression::Constant(true),
            Expression::Constant(true),
            Expression::Constant(false),
        ];
        assert!(!eval(expr).unwrap());

        // Three of three match. XOR-parity would (incorrectly) report a match here, since three is
        // odd.
        let expr = one_of![
            Expression::Constant(true),
            Expression::Constant(true),
            Expression::Constant(true),
        ];
        assert!(!eval(expr).unwrap());

        // None of three match.
        let expr = one_of![
            Expression::Constant(false),
            Expression::Constant(false),
            Expression::Constant(false),
        ];
        assert!(!eval(expr).unwrap());
    }

    #[test]
    fn test_not() {
        let expr = not!(Expression::Constant(false));
        assert!(eval(expr).unwrap());
    }

    #[test]
    fn test_constant() {
        let expr: Expression<TestMatcher> = Expression::Constant(true);
        assert!(eval(expr).unwrap());

        let expr: Expression<TestMatcher> = Expression::Constant(false);
        assert!(!eval(expr).unwrap());
    }
}
