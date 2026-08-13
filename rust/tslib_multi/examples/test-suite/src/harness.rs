use std::time::Instant;

#[derive(Debug, Clone)]
pub enum TestStatus {
    Pass,
    Fail(String),
    Skip(String),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TestResult {
    pub name: String,
    pub group: String,
    pub status: TestStatus,
    pub duration_ms: u128,
}

pub struct TestGroup {
    pub name: String,
    pub results: Vec<TestResult>,
}

impl TestGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            results: Vec::new(),
        }
    }

    pub fn run_test(&mut self, name: &str, test_fn: impl FnOnce() -> Result<(), String>) {
        let start = Instant::now();
        let status = match test_fn() {
            Ok(()) => TestStatus::Pass,
            Err(e) => TestStatus::Fail(e),
        };
        let duration_ms = start.elapsed().as_millis();
        self.results.push(TestResult {
            name: name.to_string(),
            group: self.name.clone(),
            status,
            duration_ms,
        });
    }

    pub async fn run_test_async<F, Fut>(&mut self, name: &str, test_fn: F)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        let start = Instant::now();
        let status = match test_fn().await {
            Ok(()) => TestStatus::Pass,
            Err(e) => TestStatus::Fail(e),
        };
        let duration_ms = start.elapsed().as_millis();
        self.results.push(TestResult {
            name: name.to_string(),
            group: self.name.clone(),
            status,
            duration_ms,
        });
    }

    pub fn skip_test(&mut self, name: &str, reason: &str) {
        self.results.push(TestResult {
            name: name.to_string(),
            group: self.name.clone(),
            status: TestStatus::Skip(reason.to_string()),
            duration_ms: 0,
        });
    }
}

pub struct TestSuite {
    pub groups: Vec<TestGroup>,
}

impl TestSuite {
    pub fn new() -> Self {
        Self { groups: Vec::new() }
    }

    pub fn add_group(&mut self, group: TestGroup) {
        self.groups.push(group);
    }

    pub fn print_results(&self) {
        let mut total_pass = 0usize;
        let mut total_fail = 0usize;
        let mut total_skip = 0usize;

        println!("\n{}", "=".repeat(60));
        println!("  TEST RESULTS");
        println!("{}", "=".repeat(60));

        for group in &self.groups {
            println!("\n  [{}]", group.name);
            for result in &group.results {
                let (icon, detail) = match &result.status {
                    TestStatus::Pass => {
                        total_pass += 1;
                        ("\x1b[32mPASS\x1b[0m", String::new())
                    }
                    TestStatus::Fail(e) => {
                        total_fail += 1;
                        ("\x1b[31mFAIL\x1b[0m", format!("  \x1b[31m-> {}\x1b[0m", e))
                    }
                    TestStatus::Skip(r) => {
                        total_skip += 1;
                        ("\x1b[33mSKIP\x1b[0m", format!("  \x1b[33m-> {}\x1b[0m", r))
                    }
                };
                println!(
                    "    {} {} ({}ms){}",
                    icon, result.name, result.duration_ms, detail
                );
            }
        }

        let total = total_pass + total_fail + total_skip;
        println!("\n{}", "=".repeat(60));
        println!(
            "  Total: {}  |  \x1b[32m{} passed\x1b[0m  |  \x1b[31m{} failed\x1b[0m  |  \x1b[33m{} skipped\x1b[0m",
            total, total_pass, total_fail, total_skip
        );
        println!("{}", "=".repeat(60));

        if total_fail > 0 {
            std::process::exit(1);
        }
    }

    pub fn filter(&mut self, pattern: &str) {
        for group in &mut self.groups {
            group.results.retain(|r| r.name.contains(pattern));
        }
        self.groups.retain(|g| !g.results.is_empty());
    }
}
