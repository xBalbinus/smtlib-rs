#![cfg_attr(feature = "const-bit-vec", feature(generic_const_exprs))]
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

use std::collections::HashMap;

use itertools::Itertools;
use smtlib_lowlevel::ast;
use terms::Const;
pub use terms::Sort;

pub use backend::Backend;
pub use logics::Logic;
pub use smtlib_lowlevel::backend;

#[cfg(feature = "async")]
mod async_solver;
mod logics;
mod solver;
pub mod terms;
pub mod theories;

#[cfg(feature = "async")]
pub use async_solver::AsyncSolver;
pub use solver::Solver;
pub use theories::{core::*, fixed_size_bit_vectors::*, ints::*, reals::*};

/// The satisfiability result produced by a solver
#[derive(Debug)]
pub enum SatResult {
    /// The solver produced `unsat`
    Unsat,
    /// The solver produced `sat`
    Sat,
    /// The solver produced `unknown`
    Unknown,
}

impl std::fmt::Display for SatResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SatResult::Unsat => write!(f, "unsat"),
            SatResult::Sat => write!(f, "sat"),
            SatResult::Unknown => write!(f, "unknown"),
        }
    }
}

/// The satisfiability result produced by a solver, where the [`Sat`](SatResultWithModel::Sat) variant
/// carries the satisfying model as well.
#[derive(Debug)]
pub enum SatResultWithModel {
    /// The solver produced `unsat`
    Unsat,
    /// The solver produced `sat` and the associated model
    Sat(Model),
    /// The solver produced `unknown`
    Unknown,
}

impl SatResultWithModel {
    /// Expect the result to be `sat` returning the satisfying model. If not
    /// `sat`, returns an error.
    pub fn expect_sat(self) -> Result<Model, Error> {
        match self {
            SatResultWithModel::Sat(m) => Ok(m),
            SatResultWithModel::Unsat => Err(Error::UnexpectedSatResult {
                expected: SatResult::Sat,
                actual: SatResult::Unsat,
            }),
            SatResultWithModel::Unknown => Err(Error::UnexpectedSatResult {
                expected: SatResult::Sat,
                actual: SatResult::Unknown,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::terms::{forall, Sort};

    use super::*;

    #[test]
    fn int_math() {
        let x = Int::from_name("x");
        let y = Int::from_name("hello");
        // let x_named = x.labeled();
        let mut z = 12 + y * 4;
        z += 3;
        let w = x * x + z;
        println!("{w}");
    }

    #[test]
    fn quantifiers() {
        let x = Int::from_name("x");
        let y = Int::from_name("y");

        let res = forall((x, y), (x + 2)._eq(y));
        println!("{}", ast::Term::from(res));
    }
}

/// An error that occurred during any stage of using `smtlib`.
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    /// An error originating from the low-level crate. These can for example be
    /// parsing errors, or solvers occurring with interacting the the solvers.
    Lowlevel(
        #[from]
        #[diagnostic_source]
        smtlib_lowlevel::Error,
    ),
    #[error("SMT error {0} after running {1}")]
    /// An error produced by the SMT solver. These are of the form
    ///
    /// ```ignore
    /// (error "the error goes here")
    /// ```
    Smt(String, String),
    #[error("Expected the model to be {expected} but was {actual}")]
    /// Can occur by calling [`SatResultWithModel::expect_sat`] for example.
    UnexpectedSatResult {
        /// The expected sat result
        expected: SatResult,
        /// The actual sat result
        actual: SatResult,
    },
}

/// A [`Model`] contains the values of all named constants returned through
/// [`Solver::check_sat_with_model`] or by calling [`Solver::get_model`].
///
/// The two most common usages of [`Model`] is to:
/// - Extract values for constants using [`Model::eval`].
/// - Print out the produced model using `println!("{model}")`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Model {
    values: HashMap<String, ast::Term>,
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.values.fmt(f)
    }
}
impl std::fmt::Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{ {} }}",
            self.values
                .iter()
                .map(|(n, t)| format!("{n}: {t}"))
                .format(", ")
        )
    }
}

impl Model {
    fn new(model: ast::GetModelResponse) -> Self {
        Self {
            values: model
                .0
                .into_iter()
                .map(|res| match res {
                    ast::ModelResponse::DefineFun(f) => (f.0 .0.trim_matches('|').into(), f.3),
                    ast::ModelResponse::DefineFunRec(_) => todo!(),
                    ast::ModelResponse::DefineFunsRec(_, _) => todo!(),
                })
                .collect(),
        }
    }
    /// Extract the value of a constant. Returns `None` if the value was not
    /// part of the model, which occurs if the constant was not part of any
    /// expression asserted.
    ///
    /// ```
    /// # use smtlib::{Int, Sort};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut solver = smtlib::Solver::new(smtlib::backend::Z3Binary::new("z3")?)?;
    /// let x = Int::from_name("x");
    /// solver.assert(x._eq(12))?;
    /// let model = solver.check_sat_with_model()?.expect_sat()?;
    /// match model.eval(x) {
    ///     Some(x) => println!("This is the value of x: {x}"),
    ///     None => panic!("Oh no! This should never happen, as x was part of an assert"),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn eval<T: Sort>(&self, x: Const<T>) -> Option<T::Inner>
    where
        T::Inner: From<ast::Term>,
    {
        Some(self.values.get(x.name().trim_matches('|'))?.clone().into())
    }
}
