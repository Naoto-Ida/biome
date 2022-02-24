pub mod compare;
mod reporters;
pub mod results;
mod runner;
pub mod test262;
pub mod typescript;
mod util;

pub use crate::reporters::SummaryDetailLevel;

use crate::reporters::{
    DefaultReporter, JsonReporter, MulticastTestReporter, OutputTarget, SummaryReporter,
    TestReporter,
};
use crate::runner::{run_test_suite, TestRunContext, TestSuite};
use crate::test262::Test262TestSuite;
use crate::typescript::TypeScriptTestSuite;
use rslint_parser::ParserError;
use serde::{Deserialize, Serialize};
use std::any::Any;

#[derive(Debug, Serialize, Deserialize)]
pub struct TestResult {
    #[serde(rename = "o")]
    pub outcome: Outcome,
    #[serde(rename = "h")]
    pub test_case: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Copy, Clone)]
pub enum Outcome {
    Passed,
    Failed,
    Panicked,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestResults {
    #[serde(rename = "s")]
    pub summary: Summary,
    #[serde(rename = "p")]
    pub details: Vec<TestResult>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Summary {
    #[serde(rename = "a")]
    pub tests_ran: u32,
    #[serde(rename = "pa")]
    pub passed: u32,
    #[serde(rename = "f")]
    pub failed: u32,
    #[serde(rename = "pc")]
    pub panics: u32,
    #[serde(rename = "c")]
    pub coverage: f64,
}

impl Default for TestResults {
    fn default() -> Self {
        Self {
            summary: Summary {
                tests_ran: 0,
                passed: 0,
                failed: 0,
                panics: 0,
                coverage: 0.0,
            },
            details: vec![],
        }
    }
}

impl TestResults {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store_results(&mut self, results: Vec<TestResult>) {
        self.details = results;
        let passed = self.passed_tests() as u32;
        let tests_ran = self.details.len();
        let coverage = (passed as f64 / tests_ran as f64) * 100.0;
        self.summary = Summary {
            tests_ran: self.details.len() as u32,
            passed,
            failed: self.errored_tests() as u32,
            panics: self.panicked_tests() as u32,
            coverage,
        };
    }

    pub fn panicked_tests(&self) -> usize {
        self.details
            .iter()
            .filter(|res| matches!(res.outcome, Outcome::Panicked))
            .count()
    }

    pub fn errored_tests(&self) -> usize {
        self.details
            .iter()
            .filter(|res| matches!(res.outcome, Outcome::Failed))
            .count()
    }

    pub fn passed_tests(&self) -> usize {
        self.details
            .iter()
            .filter(|res| res.outcome == Outcome::Passed)
            .count()
    }
}

pub fn run(
    language: Option<&str>,
    filter: Option<&str>,
    json: bool,
    detail_level: SummaryDetailLevel,
) {
    let mut reporters = MulticastTestReporter::new(Box::new(DefaultReporter::default()));

    let output_target = if json {
        reporters.add(Box::new(JsonReporter::default()));
        OutputTarget::stderr()
    } else {
        OutputTarget::stdout()
    };

    reporters.add(Box::new(SummaryReporter::new(detail_level, output_target)));

    let mut context = TestRunContext {
        filter: filter.map(|s| s.to_string()),
        reporter: &mut reporters,
        pool: &yastl::Pool::new(num_cpus::get().max(2)),
    };

    let mut ran_any_tests = false;
    for test_suite in get_test_suites(language) {
        let result = run_test_suite(test_suite.as_ref(), &mut context);
        ran_any_tests = ran_any_tests || result.summary.tests_ran > 0
    }

    reporters.run_completed();

    if !ran_any_tests {
        std::process::exit(1);
    }
}

fn get_test_suites(language: Option<&str>) -> Vec<Box<dyn TestSuite>> {
    language
        .map(|language| {
            let test_suite: Box<dyn TestSuite> = match language.to_lowercase().as_str() {
                "js" | "javascript" => Box::new(Test262TestSuite),
                "ts" | "typescript" => Box::new(TypeScriptTestSuite),
                other => panic!("Unknown language: {}", other),
            };

            vec![test_suite]
        })
        .unwrap_or_else(|| vec![Box::new(Test262TestSuite), Box::new(TypeScriptTestSuite)])
}