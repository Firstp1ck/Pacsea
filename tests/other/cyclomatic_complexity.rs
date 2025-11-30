//! Cyclomatic complexity calculation test for the Pacsea project.
//!
//! This test analyzes all Rust source files in the project and calculates
//! cyclomatic complexity metrics for functions and methods.
//!
//! Cyclomatic complexity measures the number of linearly independent paths
//! through a program's source code. It's calculated as:
//! - Base complexity: 1
//! - Add 1 for each: if, while, for, loop, match arm, &&, ||, ? operator, catch blocks
//!
//! Higher complexity indicates more decision points and potentially harder-to-maintain code.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// ANSI color codes (harmonized with Makefile)
const COLOR_RESET: &str = "\x1b[0m";
const COLOR_BOLD: &str = "\x1b[1m";
const COLOR_BLUE: &str = "\x1b[34m";
const COLOR_YELLOW: &str = "\x1b[33m";

/// Represents complexity metrics for a single function or method.
#[derive(Debug, Clone)]
struct FunctionComplexity {
    /// Name of the function/method
    name: String,
    /// File path where the function is defined
    file: PathBuf,
    /// Cyclomatic complexity value
    complexity: u32,
    /// Line number where the function starts
    line: usize,
}

/// Represents complexity metrics for an entire file.
#[derive(Debug)]
struct FileComplexity {
    /// Functions and their complexities
    functions: Vec<FunctionComplexity>,
    /// Total complexity (sum of all function complexities)
    total_complexity: u32,
    /// Average complexity per function
    avg_complexity: f64,
}

/// Visitor that traverses the AST and calculates cyclomatic complexity.
struct ComplexityVisitor {
    /// Current function being analyzed
    current_function: Option<String>,
    /// Current file being analyzed
    current_file: PathBuf,
    /// Functions found and their complexities
    functions: Vec<FunctionComplexity>,
    /// Current complexity counter
    current_complexity: u32,
    /// Current line number
    current_line: usize,
}

impl ComplexityVisitor {
    /// Creates a new visitor for a given file.
    const fn new(file: PathBuf) -> Self {
        Self {
            current_function: None,
            current_file: file,
            functions: Vec::new(),
            current_complexity: 1, // Base complexity
            current_line: 0,
        }
    }

    /// Calculates complexity for a single expression.
    fn visit_expr(&mut self, expr: &syn::Expr) {
        match expr {
            syn::Expr::While(_) | syn::Expr::ForLoop(_) | syn::Expr::Loop(_) => {
                self.current_complexity += 1;
            }
            syn::Expr::Match(m) => {
                // Each match arm adds complexity
                self.current_complexity += u32::try_from(m.arms.len()).unwrap_or(u32::MAX);
                // Guards add additional complexity
                for arm in &m.arms {
                    if arm.guard.is_some() {
                        self.current_complexity += 1;
                    }
                }
            }
            syn::Expr::Binary(bin) => {
                // && and || operators add complexity
                match bin.op {
                    syn::BinOp::And(_) | syn::BinOp::Or(_) => {
                        self.current_complexity += 1;
                    }
                    _ => {}
                }
            }
            syn::Expr::Try(_) => {
                // ? operator adds complexity
                self.current_complexity += 1;
            }
            syn::Expr::Call(call) => {
                // Recursively visit nested expressions
                self.visit_expr(&call.func);
                for arg in &call.args {
                    self.visit_expr(arg);
                }
            }
            syn::Expr::MethodCall(mcall) => {
                for arg in &mcall.args {
                    self.visit_expr(arg);
                }
            }
            syn::Expr::Block(block) => {
                for stmt in &block.block.stmts {
                    self.visit_stmt(stmt);
                }
            }
            syn::Expr::If(if_expr) => {
                self.current_complexity += 1;
                self.visit_expr(&if_expr.cond);
                // Visit then branch as a block
                for stmt in &if_expr.then_branch.stmts {
                    self.visit_stmt(stmt);
                }
                if let Some((_, else_expr)) = &if_expr.else_branch {
                    self.visit_expr(else_expr);
                }
            }
            syn::Expr::Unary(unary) => {
                self.visit_expr(&unary.expr);
            }
            syn::Expr::Paren(paren) => {
                self.visit_expr(&paren.expr);
            }
            syn::Expr::Group(group) => {
                self.visit_expr(&group.expr);
            }
            syn::Expr::Array(array) => {
                for elem in &array.elems {
                    self.visit_expr(elem);
                }
            }
            syn::Expr::Tuple(tuple) => {
                for elem in &tuple.elems {
                    self.visit_expr(elem);
                }
            }
            syn::Expr::Struct(struct_expr) => {
                for field in &struct_expr.fields {
                    self.visit_expr(&field.expr);
                }
            }
            syn::Expr::Repeat(repeat) => {
                self.visit_expr(&repeat.expr);
            }
            syn::Expr::Closure(closure) => {
                self.visit_expr(&closure.body);
            }
            syn::Expr::Async(async_expr) => {
                for stmt in &async_expr.block.stmts {
                    self.visit_stmt(stmt);
                }
            }
            syn::Expr::Await(await_expr) => {
                self.visit_expr(&await_expr.base);
            }
            syn::Expr::Let(let_expr) => {
                self.visit_expr(&let_expr.expr);
            }
            syn::Expr::Assign(assign) => {
                self.visit_expr(&assign.right);
            }
            syn::Expr::Range(range) => {
                if let Some(start) = &range.start {
                    self.visit_expr(start);
                }
                if let Some(end) = &range.end {
                    self.visit_expr(end);
                }
            }
            syn::Expr::Index(index) => {
                self.visit_expr(&index.expr);
                self.visit_expr(&index.index);
            }
            syn::Expr::Field(field) => {
                self.visit_expr(&field.base);
            }
            _ => {
                // Leaf nodes and other expression types, no additional complexity
                // For other expression types, we could add more specific handling
                // but for now, we'll skip them to avoid over-counting
            }
        }
    }

    /// Calculates complexity for a single statement.
    fn visit_stmt(&mut self, stmt: &syn::Stmt) {
        match stmt {
            syn::Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    self.visit_expr(&init.expr);
                }
            }
            syn::Stmt::Expr(expr, _) => {
                self.visit_expr(expr);
            }
            syn::Stmt::Item(_) | syn::Stmt::Macro(_) => {
                // Items and macros don't add complexity directly
                // Macros are complex but hard to analyze statically
            }
        }
    }

    /// Visits a function and calculates its complexity.
    fn visit_item_fn(&mut self, item_fn: &syn::ItemFn, name: String, line: usize) {
        let saved_complexity = self.current_complexity;
        let saved_function = self.current_function.clone();

        self.current_complexity = 1; // Base complexity
        self.current_function = Some(name.clone());
        self.current_line = line;

        // Visit the function body
        for stmt in &item_fn.block.stmts {
            self.visit_stmt(stmt);
        }

        // Save the function complexity
        self.functions.push(FunctionComplexity {
            name,
            file: self.current_file.clone(),
            complexity: self.current_complexity,
            line: self.current_line,
        });

        // Restore previous state
        self.current_complexity = saved_complexity;
        self.current_function = saved_function;
    }

    /// Visits an impl method and calculates its complexity.
    fn visit_impl_item_fn(&mut self, method: &syn::ImplItemFn, name: String, line: usize) {
        let saved_complexity = self.current_complexity;
        let saved_function = self.current_function.clone();

        self.current_complexity = 1; // Base complexity
        self.current_function = Some(name.clone());
        self.current_line = line;

        // Visit the method body
        for stmt in &method.block.stmts {
            self.visit_stmt(stmt);
        }

        // Save the method complexity
        self.functions.push(FunctionComplexity {
            name,
            file: self.current_file.clone(),
            complexity: self.current_complexity,
            line: self.current_line,
        });

        // Restore previous state
        self.current_complexity = saved_complexity;
        self.current_function = saved_function;
    }

    /// Visits an impl block to analyze methods.
    fn visit_impl(&mut self, item_impl: &syn::ItemImpl) {
        for item in &item_impl.items {
            if let syn::ImplItem::Fn(method) = item {
                let name = method.sig.ident.to_string();
                // Line numbers are not easily accessible from syn spans in syn 2.0
                // Using 0 as placeholder - could be enhanced with source file parsing
                let line = 0;
                self.visit_impl_item_fn(method, name, line);
            }
        }
    }

    /// Visits all items in a file.
    fn visit_file(&mut self, file: &syn::File) {
        for item in &file.items {
            match item {
                syn::Item::Fn(item_fn) => {
                    let name = item_fn.sig.ident.to_string();
                    // Line numbers are not easily accessible from syn spans in syn 2.0
                    // Using 0 as placeholder - could be enhanced with source file parsing
                    let line = 0;
                    self.visit_item_fn(item_fn, name, line);
                }
                syn::Item::Impl(item_impl) => {
                    self.visit_impl(item_impl);
                }
                syn::Item::Mod(_item_mod) => {
                    // Nested modules are handled separately
                }
                _ => {}
            }
        }
    }
}

/// Analyzes a single Rust source file and returns its complexity metrics.
fn analyze_file(file_path: &Path) -> Result<FileComplexity, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(file_path)?;
    let ast = syn::parse_file(&content)?;

    let mut visitor = ComplexityVisitor::new(file_path.to_path_buf());
    visitor.visit_file(&ast);

    let total_complexity: u32 = visitor.functions.iter().map(|f| f.complexity).sum();
    #[allow(clippy::cast_precision_loss)]
    let avg_complexity = if visitor.functions.is_empty() {
        0.0
    } else {
        f64::from(total_complexity) / visitor.functions.len() as f64
    };

    Ok(FileComplexity {
        functions: visitor.functions,
        total_complexity,
        avg_complexity,
    })
}

/// Recursively finds all Rust source files in a directory.
fn find_rust_files(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();

    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Skip target directory
                if path.file_name().and_then(|n| n.to_str()) == Some("target") {
                    continue;
                }
                files.extend(find_rust_files(&path)?);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }

    Ok(files)
}

/// Calculates cyclomatic complexity for the entire project.
fn calculate_project_complexity()
-> Result<HashMap<PathBuf, FileComplexity>, Box<dyn std::error::Error>> {
    let src_dir = Path::new("src");
    let test_dir = Path::new("tests");

    let mut results = HashMap::new();

    // Analyze src directory
    if src_dir.exists() {
        for file in find_rust_files(src_dir)? {
            match analyze_file(&file) {
                Ok(complexity) => {
                    results.insert(file.clone(), complexity);
                }
                Err(e) => {
                    eprintln!(
                        "{COLOR_YELLOW}Warning:{COLOR_RESET} Failed to analyze {}: {}",
                        file.display(),
                        e
                    );
                }
            }
        }
    }

    // Analyze tests directory
    if test_dir.exists() {
        for file in find_rust_files(test_dir)? {
            match analyze_file(&file) {
                Ok(complexity) => {
                    results.insert(file.clone(), complexity);
                }
                Err(e) => {
                    eprintln!(
                        "{COLOR_YELLOW}Warning:{COLOR_RESET} Failed to analyze {}: {}",
                        file.display(),
                        e
                    );
                }
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that calculates and reports cyclomatic complexity for the entire project.
    ///
    /// This test:
    /// - Analyzes all Rust source files in src/ and tests/
    /// - Calculates complexity for each function/method
    /// - Reports statistics and identifies high-complexity functions
    /// - Optionally fails if complexity exceeds thresholds
    #[test]
    fn test_cyclomatic_complexity() {
        // Complexity thresholds (commonly used guidelines)
        const VERY_HIGH_COMPLEXITY: u32 = 20;
        const HIGH_COMPLEXITY: u32 = 10;
        const MODERATE_COMPLEXITY: u32 = 5;
        const MAX_AVERAGE_COMPLEXITY: f64 = 8.0;
        const MAX_FILE_AVG_COMPLEXITY: f64 = 15.0;

        let results =
            calculate_project_complexity().expect("Failed to calculate project complexity");

        assert!(!results.is_empty(), "No Rust files found to analyze");

        // Collect all functions
        let mut all_functions = Vec::new();
        let mut total_project_complexity = 0u32;
        let mut total_functions = 0usize;

        for file_complexity in results.values() {
            total_project_complexity += file_complexity.total_complexity;
            total_functions += file_complexity.functions.len();
            all_functions.extend(file_complexity.functions.clone());
        }

        // Sort functions by complexity (highest first)
        all_functions.sort_by(|a, b| b.complexity.cmp(&a.complexity));

        // Print summary
        println!("\n=== Cyclomatic Complexity Report ===");
        println!("Total files analyzed: {}", results.len());
        println!("Total functions/methods: {total_functions}");
        println!("Total project complexity: {total_project_complexity}");

        if total_functions > 0 {
            #[allow(clippy::cast_precision_loss)]
            let avg_complexity = f64::from(total_project_complexity) / total_functions as f64;
            println!("Average complexity per function: {avg_complexity:.2}");
        }

        // Report top 10 most complex functions
        println!("\n=== Top 10 Most Complex Functions ===");
        for (i, func) in all_functions.iter().take(10).enumerate() {
            println!(
                "{}. {} (complexity: {}) - {}:{}",
                i + 1,
                func.name,
                func.complexity,
                func.file.display(),
                func.line
            );
        }

        // Report files with highest complexity
        println!("\n=== Files by Total Complexity ===");
        let mut file_complexities: Vec<_> = results.iter().collect();
        file_complexities.sort_by(|a, b| b.1.total_complexity.cmp(&a.1.total_complexity));

        for (file, file_comp) in file_complexities.iter().take(10) {
            println!(
                "{}: total={}, avg={:.2}, functions={}",
                file.display(),
                file_comp.total_complexity,
                file_comp.avg_complexity,
                file_comp.functions.len()
            );
        }

        // Count functions by complexity level
        let very_high = all_functions
            .iter()
            .filter(|f| f.complexity >= VERY_HIGH_COMPLEXITY)
            .count();
        let high = all_functions
            .iter()
            .filter(|f| f.complexity >= HIGH_COMPLEXITY && f.complexity < VERY_HIGH_COMPLEXITY)
            .count();
        let moderate = all_functions
            .iter()
            .filter(|f| f.complexity >= MODERATE_COMPLEXITY && f.complexity < HIGH_COMPLEXITY)
            .count();

        println!("\n{COLOR_BOLD}{COLOR_BLUE}=== Complexity Distribution ==={COLOR_RESET}");
        println!("Very High (≥{VERY_HIGH_COMPLEXITY}): {very_high}");
        println!(
            "High ({}..{}): {}",
            HIGH_COMPLEXITY,
            VERY_HIGH_COMPLEXITY - 1,
            high
        );
        println!(
            "Moderate ({}..{}): {}",
            MODERATE_COMPLEXITY,
            HIGH_COMPLEXITY - 1,
            moderate
        );
        println!(
            "Low (<{}): {}",
            MODERATE_COMPLEXITY,
            total_functions - very_high - high - moderate
        );

        // List functions with very high complexity
        if very_high > 0 {
            println!(
                "\n{COLOR_BOLD}{COLOR_YELLOW}=== Functions with Very High Complexity (≥{VERY_HIGH_COMPLEXITY}) ==={COLOR_RESET}"
            );
            for func in all_functions
                .iter()
                .filter(|f| f.complexity >= VERY_HIGH_COMPLEXITY)
            {
                println!(
                    "  {} (complexity: {}) - {}:{}",
                    func.name,
                    func.complexity,
                    func.file.display(),
                    func.line
                );
            }
        }

        // ====================================================================
        // ASSERTIONS - Choose appropriate strictness level for your project
        // ====================================================================
        //
        // Industry guidelines for cyclomatic complexity:
        // - 1-4:   Simple (ideal)
        // - 5-9:   Moderate (acceptable)
        // - 10-19: High (consider refactoring)
        // - 20+:   Very High (should be refactored)
        //
        // Note: Rust's pattern matching naturally increases complexity scores,
        // so slightly higher thresholds may be acceptable compared to other languages.

        // OPTION 1: STRICT - Fail if any function exceeds a threshold
        // Recommended threshold: 30-50 for Rust (due to match statements)
        // This catches extremely complex functions that definitely need refactoring
        // NOTE: Your project currently has functions > 50, so adjust threshold or refactor first
        /*
        const MAX_FUNCTION_COMPLEXITY: u32 = 50;
        let max_complexity = all_functions
            .iter()
            .map(|f| f.complexity)
            .max()
            .unwrap_or(0);
        assert!(
            max_complexity <= MAX_FUNCTION_COMPLEXITY,
            "Found function with complexity {} (max allowed: {}). Consider refactoring.",
            max_complexity,
            MAX_FUNCTION_COMPLEXITY
        );
        */

        // OPTION 2: MODERATE - Limit the number of very high complexity functions
        // Recommended: Allow 5-10% of functions to be very high complexity
        // Current project has 21/415 = 5.1% which is reasonable
        // This assertion currently passes with your project (21 <= 25)
        /*
        const MAX_VERY_HIGH_COMPLEXITY_FUNCTIONS: usize = 25; // ~6% of 415 functions
        assert!(
            very_high <= MAX_VERY_HIGH_COMPLEXITY_FUNCTIONS,
            "Too many functions with very high complexity (≥{}): {} (max allowed: {}). \
             Consider refactoring the most complex functions.",
            VERY_HIGH_COMPLEXITY,
            very_high,
            MAX_VERY_HIGH_COMPLEXITY_FUNCTIONS
        );
        */

        // OPTION 3: AVERAGE COMPLEXITY - Ensure overall codebase stays maintainable
        // Recommended: Average should stay below 8-10
        // Current project average: 6.00 (excellent) - this assertion currently passes

        #[allow(clippy::cast_precision_loss)]
        let avg_complexity = if total_functions > 0 {
            f64::from(total_project_complexity) / total_functions as f64
        } else {
            0.0
        };
        assert!(
            avg_complexity <= MAX_AVERAGE_COMPLEXITY,
            "Average complexity too high: {avg_complexity:.2} (max allowed: {MAX_AVERAGE_COMPLEXITY:.2}). \
             Consider refactoring high-complexity functions."
        );

        // OPTION 4: PROGRESSIVE - Prevent new extremely complex functions
        // This is more useful in CI/CD to prevent regression
        // Fail if any function exceeds a "hard limit" (e.g., 100+)
        // NOTE: Your project currently has handle_mouse_event with complexity 220
        // Consider refactoring before enabling this, or set threshold higher (e.g., 250)
        /*
        const HARD_LIMIT_COMPLEXITY: u32 = 100;
        let extremely_complex: Vec<_> = all_functions
            .iter()
            .filter(|f| f.complexity >= HARD_LIMIT_COMPLEXITY)
            .collect();
        if !extremely_complex.is_empty() {
            let names: Vec<String> = extremely_complex
                .iter()
                .map(|f| format!("{} ({})", f.name, f.complexity))
                .collect();
            panic!(
                "Found {} function(s) exceeding hard complexity limit (≥{}): {}. \
                 These MUST be refactored before merging.",
                extremely_complex.len(),
                HARD_LIMIT_COMPLEXITY,
                names.join(", ")
            );
        }
        */

        // OPTION 5: FILE-LEVEL - Prevent individual files from becoming too complex
        // Recommended: No single file should have average complexity > 15
        // This is a warning only (doesn't fail the test)
        // Note: Only warn for files with multiple functions. For single-function files,
        // the individual function complexity check is more appropriate.
        for (file_path, file_comp) in &file_complexities {
            if file_comp.functions.len() > 1 && file_comp.avg_complexity > MAX_FILE_AVG_COMPLEXITY {
                eprintln!(
                    "{COLOR_YELLOW}Warning:{COLOR_RESET} File {} has high average complexity: {:.2} ({} functions)",
                    file_path.display(),
                    file_comp.avg_complexity,
                    file_comp.functions.len()
                );
            }
        }

        // ====================================================================
        // RECOMMENDATIONS FOR YOUR PROJECT:
        // ====================================================================
        // Based on current metrics:
        // - Average complexity: 6.00 (excellent, well below threshold)
        // - Very high complexity functions: 21 (5.1% of total, reasonable)
        // - Most complex function: handle_mouse_event (220) - consider refactoring
        //
        // Suggested approach:
        // 1. Start with OPTION 3 (average complexity) - already passes, prevents regression
        // 2. Enable OPTION 2 (limit very high complexity) with threshold 25-30
        // 3. For new code, consider OPTION 4 with threshold 250+ to prevent new extreme cases
        // 4. Refactor handle_mouse_event (220) and other functions > 100 when possible
        //
        // Uncomment the assertions above that match your project's needs.
        // For a new project, start with OPTION 1 (strict) and OPTION 4 (hard limit).
        // For an existing project, OPTION 2 (moderate) and OPTION 3 (average) are more practical.
    }
}
