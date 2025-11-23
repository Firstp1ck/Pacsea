//! Data flow complexity calculation test for the Pacsea project.
//!
//! This test analyzes all Rust source files in the project and calculates
//! data flow complexity metrics for functions and methods according to Dunsmore.
//!
//! Data flow complexity measures the complexity of data flow through a program by:
//! - Identifying variable definitions (defs) - where variables are assigned values
//! - Identifying variable uses (uses) - where variable values are accessed
//! - Counting Definition-Use (DU) pairs - paths from definitions to uses
//! - Measuring complexity based on the number of DU pairs and their nesting levels
//!
//! Higher complexity indicates more complex data dependencies and potentially harder-to-maintain code.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Represents data flow complexity metrics for a single function or method.
#[derive(Debug, Clone)]
struct FunctionDataFlowComplexity {
    /// Name of the function/method
    name: String,
    /// File path where the function is defined
    file: PathBuf,
    /// Data flow complexity value (number of DU pairs)
    complexity: u32,
    /// Number of variable definitions
    definitions: u32,
    /// Number of variable uses
    uses: u32,
    /// Line number where the function starts
    line: usize,
}

/// Represents data flow complexity metrics for an entire file.
#[derive(Debug)]
struct FileDataFlowComplexity {
    /// Functions and their complexities
    functions: Vec<FunctionDataFlowComplexity>,
    /// Total complexity (sum of all function complexities)
    total_complexity: u32,
    /// Average complexity per function
    avg_complexity: f64,
}

/// Visitor that traverses the AST and calculates data flow complexity.
struct DataFlowVisitor {
    /// Current function being analyzed
    current_function: Option<String>,
    /// Current file being analyzed
    current_file: PathBuf,
    /// Functions found and their complexities
    functions: Vec<FunctionDataFlowComplexity>,
    /// Current function's variable definitions
    current_definitions: HashSet<String>,
    /// Current function's variable uses
    current_uses: HashSet<String>,
    /// Current function's DU pairs (def-use pairs)
    current_du_pairs: HashSet<(String, String)>,
    /// Current line number
    current_line: usize,
    /// Current nesting level (for complexity weighting)
    nesting_level: u32,
}

impl DataFlowVisitor {
    /// Creates a new visitor for a given file.
    fn new(file: PathBuf) -> Self {
        Self {
            current_function: None,
            current_file: file,
            functions: Vec::new(),
            current_definitions: HashSet::new(),
            current_uses: HashSet::new(),
            current_du_pairs: HashSet::new(),
            current_line: 0,
            nesting_level: 0,
        }
    }

    /// Records a variable definition.
    fn record_def(&mut self, var_name: &str) {
        self.current_definitions.insert(var_name.to_string());
        // Create DU pairs: this definition can reach all existing uses
        for use_var in &self.current_uses {
            if use_var == var_name {
                self.current_du_pairs
                    .insert((var_name.to_string(), use_var.clone()));
            }
        }
    }

    /// Records a variable use.
    fn record_use(&mut self, var_name: &str) {
        self.current_uses.insert(var_name.to_string());
        // Create DU pairs: this use can be reached by all existing definitions
        for def_var in &self.current_definitions {
            if def_var == var_name {
                self.current_du_pairs
                    .insert((def_var.clone(), var_name.to_string()));
            }
        }
    }

    /// Calculates data flow complexity for a single expression.
    fn visit_expr(&mut self, expr: &syn::Expr) {
        match expr {
            syn::Expr::Assign(assign) => {
                // Left side is a definition
                if let syn::Expr::Path(path) = &*assign.left
                    && let Some(ident) = path.path.get_ident()
                {
                    self.record_def(&ident.to_string());
                }
                // Right side is a use
                self.visit_expr(&assign.right);
            }
            // Note: AssignOp (compound assignments like +=) are handled as Assign in syn 2.0
            // We'll handle them in the Assign case by checking if the left side is also used
            syn::Expr::Let(let_expr) => {
                // Pattern binding creates definitions
                self.visit_pat(&let_expr.pat);
                self.visit_expr(&let_expr.expr);
            }
            syn::Expr::Path(path) => {
                // Variable access is a use
                if let Some(ident) = path.path.get_ident() {
                    self.record_use(&ident.to_string());
                }
            }
            syn::Expr::Call(call) => {
                // Function call arguments are uses
                self.visit_expr(&call.func);
                for arg in &call.args {
                    self.visit_expr(arg);
                }
            }
            syn::Expr::MethodCall(mcall) => {
                // Receiver and arguments are uses
                self.visit_expr(&mcall.receiver);
                for arg in &mcall.args {
                    self.visit_expr(arg);
                }
            }
            syn::Expr::If(if_expr) => {
                self.nesting_level += 1;
                self.visit_expr(&if_expr.cond);
                // Visit then branch
                for stmt in &if_expr.then_branch.stmts {
                    self.visit_stmt(stmt);
                }
                if let Some((_, else_expr)) = &if_expr.else_branch {
                    self.visit_expr(else_expr);
                }
                self.nesting_level -= 1;
            }
            syn::Expr::While(while_expr) => {
                self.nesting_level += 1;
                self.visit_expr(&while_expr.cond);
                for stmt in &while_expr.body.stmts {
                    self.visit_stmt(stmt);
                }
                self.nesting_level -= 1;
            }
            syn::Expr::ForLoop(for_loop) => {
                self.nesting_level += 1;
                // Loop variable is a definition
                self.visit_pat(&for_loop.pat);
                self.visit_expr(&for_loop.expr);
                for stmt in &for_loop.body.stmts {
                    self.visit_stmt(stmt);
                }
                self.nesting_level -= 1;
            }
            syn::Expr::Loop(loop_expr) => {
                self.nesting_level += 1;
                for stmt in &loop_expr.body.stmts {
                    self.visit_stmt(stmt);
                }
                self.nesting_level -= 1;
            }
            syn::Expr::Match(match_expr) => {
                self.nesting_level += 1;
                self.visit_expr(&match_expr.expr);
                for arm in &match_expr.arms {
                    self.visit_pat(&arm.pat);
                    if let Some((_, guard_expr)) = &arm.guard {
                        self.visit_expr(guard_expr);
                    }
                    self.visit_expr(&arm.body);
                }
                self.nesting_level -= 1;
            }
            syn::Expr::Block(block) => {
                for stmt in &block.block.stmts {
                    self.visit_stmt(stmt);
                }
            }
            syn::Expr::Binary(bin) => {
                self.visit_expr(&bin.left);
                self.visit_expr(&bin.right);
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
                // Closure parameters are definitions
                // In syn 2.0, closure.inputs is Vec<Pat>
                for input in &closure.inputs {
                    self.visit_pat(input);
                }
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
                // Leaf nodes and other expression types, no variable access
                // For other expression types, we could add more specific handling
            }
        }
    }

    /// Calculates data flow complexity for a pattern (used in let bindings, match arms, etc.).
    fn visit_pat(&mut self, pat: &syn::Pat) {
        match pat {
            syn::Pat::Ident(pat_ident) => {
                self.record_def(&pat_ident.ident.to_string());
            }
            syn::Pat::Struct(struct_pat) => {
                for field in &struct_pat.fields {
                    self.visit_pat(&field.pat);
                }
            }
            syn::Pat::Tuple(tuple_pat) => {
                for elem in &tuple_pat.elems {
                    self.visit_pat(elem);
                }
            }
            syn::Pat::Slice(slice_pat) => {
                for elem in &slice_pat.elems {
                    self.visit_pat(elem);
                }
            }
            syn::Pat::Or(or_pat) => {
                for pat in &or_pat.cases {
                    self.visit_pat(pat);
                }
            }
            _ => {
                // Other patterns don't create variable bindings we track
            }
        }
    }

    /// Calculates data flow complexity for a single statement.
    fn visit_stmt(&mut self, stmt: &syn::Stmt) {
        match stmt {
            syn::Stmt::Local(local) => {
                self.visit_pat(&local.pat);
                if let Some(init) = &local.init {
                    self.visit_expr(&init.expr);
                }
            }
            syn::Stmt::Expr(expr, _) => {
                self.visit_expr(expr);
            }
            syn::Stmt::Item(_) | syn::Stmt::Macro(_) => {
                // Items and macros don't add data flow complexity directly
                // Macros are complex but hard to analyze statically
            }
        }
    }

    /// Visits a function and calculates its data flow complexity.
    fn visit_item_fn(&mut self, item_fn: &syn::ItemFn, name: String, line: usize) {
        let saved_definitions = self.current_definitions.clone();
        let saved_uses = self.current_uses.clone();
        let saved_du_pairs = self.current_du_pairs.clone();
        let saved_function = self.current_function.clone();
        let saved_nesting = self.nesting_level;

        // Reset for new function
        self.current_definitions.clear();
        self.current_uses.clear();
        self.current_du_pairs.clear();
        self.current_function = Some(name.clone());
        self.current_line = line;
        self.nesting_level = 0;

        // Function parameters are definitions
        for input in &item_fn.sig.inputs {
            match input {
                syn::FnArg::Receiver(_) => {
                    // self is a use (we're using the receiver)
                    self.record_use("self");
                }
                syn::FnArg::Typed(typed) => {
                    self.visit_pat(&typed.pat);
                }
            }
        }

        // Visit the function body
        for stmt in &item_fn.block.stmts {
            self.visit_stmt(stmt);
        }

        // Calculate complexity: number of DU pairs, weighted by nesting level
        #[allow(clippy::cast_possible_truncation)]
        let base_complexity = self.current_du_pairs.len() as u32;
        // Add complexity for nesting (more nested = more complex data flow)
        let nesting_complexity = self.nesting_level * 2;
        let total_complexity = base_complexity + nesting_complexity;

        // Save the function complexity
        #[allow(clippy::cast_possible_truncation)]
        self.functions.push(FunctionDataFlowComplexity {
            name,
            file: self.current_file.clone(),
            complexity: total_complexity,
            definitions: self.current_definitions.len() as u32,
            uses: self.current_uses.len() as u32,
            line: self.current_line,
        });

        // Restore previous state
        self.current_definitions = saved_definitions;
        self.current_uses = saved_uses;
        self.current_du_pairs = saved_du_pairs;
        self.current_function = saved_function;
        self.nesting_level = saved_nesting;
    }

    /// Visits an impl method and calculates its data flow complexity.
    fn visit_impl_item_fn(&mut self, method: &syn::ImplItemFn, name: String, line: usize) {
        let saved_definitions = self.current_definitions.clone();
        let saved_uses = self.current_uses.clone();
        let saved_du_pairs = self.current_du_pairs.clone();
        let saved_function = self.current_function.clone();
        let saved_nesting = self.nesting_level;

        // Reset for new method
        self.current_definitions.clear();
        self.current_uses.clear();
        self.current_du_pairs.clear();
        self.current_function = Some(name.clone());
        self.current_line = line;
        self.nesting_level = 0;

        // Method parameters are definitions
        for input in &method.sig.inputs {
            match input {
                syn::FnArg::Receiver(_) => {
                    // self is a use (we're using the receiver)
                    self.record_use("self");
                }
                syn::FnArg::Typed(typed) => {
                    self.visit_pat(&typed.pat);
                }
            }
        }

        // Visit the method body
        for stmt in &method.block.stmts {
            self.visit_stmt(stmt);
        }

        // Calculate complexity: number of DU pairs, weighted by nesting level
        #[allow(clippy::cast_possible_truncation)]
        let base_complexity = self.current_du_pairs.len() as u32;
        // Add complexity for nesting (more nested = more complex data flow)
        let nesting_complexity = self.nesting_level * 2;
        let total_complexity = base_complexity + nesting_complexity;

        // Save the method complexity
        #[allow(clippy::cast_possible_truncation)]
        self.functions.push(FunctionDataFlowComplexity {
            name,
            file: self.current_file.clone(),
            complexity: total_complexity,
            definitions: self.current_definitions.len() as u32,
            uses: self.current_uses.len() as u32,
            line: self.current_line,
        });

        // Restore previous state
        self.current_definitions = saved_definitions;
        self.current_uses = saved_uses;
        self.current_du_pairs = saved_du_pairs;
        self.current_function = saved_function;
        self.nesting_level = saved_nesting;
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

/// Analyzes a single Rust source file and returns its data flow complexity metrics.
fn analyze_file(file_path: &Path) -> Result<FileDataFlowComplexity, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(file_path)?;
    let ast = syn::parse_file(&content)?;

    let mut visitor = DataFlowVisitor::new(file_path.to_path_buf());
    visitor.visit_file(&ast);

    let total_complexity: u32 = visitor.functions.iter().map(|f| f.complexity).sum();
    #[allow(clippy::cast_precision_loss)]
    let avg_complexity = if visitor.functions.is_empty() {
        0.0
    } else {
        f64::from(total_complexity) / visitor.functions.len() as f64
    };

    Ok(FileDataFlowComplexity {
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

/// Calculates data flow complexity for the entire project.
fn calculate_project_data_flow_complexity()
-> Result<HashMap<PathBuf, FileDataFlowComplexity>, Box<dyn std::error::Error>> {
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
                    eprintln!("Warning: Failed to analyze {}: {}", file.display(), e);
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
                    eprintln!("Warning: Failed to analyze {}: {}", file.display(), e);
                }
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that calculates and reports data flow complexity for the entire project.
    ///
    /// This test:
    /// - Analyzes all Rust source files in src/ and tests/
    /// - Calculates data flow complexity for each function/method according to Dunsmore
    /// - Tracks variable definitions, uses, and Definition-Use (DU) pairs
    /// - Reports statistics and identifies high-complexity functions
    /// - Optionally fails if complexity exceeds thresholds
    #[test]
    fn test_data_flow_complexity() {
        // Complexity thresholds (guidelines for data flow complexity)
        const VERY_HIGH_COMPLEXITY: u32 = 50;
        const HIGH_COMPLEXITY: u32 = 25;
        const MODERATE_COMPLEXITY: u32 = 10;
        const MAX_AVERAGE_COMPLEXITY: f64 = 8.0;
        const MAX_FILE_AVG_COMPLEXITY: f64 = 40.0;

        let results = calculate_project_data_flow_complexity()
            .expect("Failed to calculate project data flow complexity");

        assert!(!results.is_empty(), "No Rust files found to analyze");

        // Collect all functions
        let mut all_functions = Vec::new();
        let mut total_project_complexity = 0u32;
        let mut total_functions = 0usize;
        let mut total_definitions = 0u32;
        let mut total_uses = 0u32;

        for file_complexity in results.values() {
            total_project_complexity += file_complexity.total_complexity;
            total_functions += file_complexity.functions.len();
            for func in &file_complexity.functions {
                total_definitions += func.definitions;
                total_uses += func.uses;
            }
            all_functions.extend(file_complexity.functions.clone());
        }

        // Sort functions by complexity (highest first)
        all_functions.sort_by(|a, b| b.complexity.cmp(&a.complexity));

        // Print summary
        println!("\n=== Data Flow Complexity Report (Dunsmore) ===");
        println!("Total files analyzed: {}", results.len());
        println!("Total functions/methods: {total_functions}");
        println!("Total project complexity: {total_project_complexity}");
        println!("Total variable definitions: {total_definitions}");
        println!("Total variable uses: {total_uses}");

        if total_functions > 0 {
            #[allow(clippy::cast_precision_loss)]
            let avg_complexity = f64::from(total_project_complexity) / total_functions as f64;
            println!("Average complexity per function: {avg_complexity:.2}");
        }

        // Report top 10 most complex functions
        println!("\n=== Top 10 Most Complex Functions ===");
        for (i, func) in all_functions.iter().take(10).enumerate() {
            println!(
                "{}. {} (complexity: {}, defs: {}, uses: {}) - {}:{}",
                i + 1,
                func.name,
                func.complexity,
                func.definitions,
                func.uses,
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

        println!("\n=== Complexity Distribution ===");
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
            println!("\n=== Functions with Very High Complexity (≥{VERY_HIGH_COMPLEXITY}) ===");
            for func in all_functions
                .iter()
                .filter(|f| f.complexity >= VERY_HIGH_COMPLEXITY)
            {
                println!(
                    "  {} (complexity: {}, defs: {}, uses: {}) - {}:{}",
                    func.name,
                    func.complexity,
                    func.definitions,
                    func.uses,
                    func.file.display(),
                    func.line
                );
            }
        }

        // ====================================================================
        // ASSERTIONS - Choose appropriate strictness level for your project
        // ====================================================================
        //
        // Guidelines for data flow complexity (Dunsmore):
        // - Measures Definition-Use (DU) pairs and nesting levels
        // - Higher values indicate more complex data dependencies
        // - Functions with many variables and complex data flow paths score higher
        //
        // Note: Data flow complexity can be higher than cyclomatic complexity
        // because it tracks all variable interactions, not just control flow.

        // OPTION 1: AVERAGE COMPLEXITY - Ensure overall codebase stays maintainable
        // Recommended: Average should stay below 20-30 for data flow complexity
        #[allow(clippy::cast_precision_loss)]
        let avg_complexity = if total_functions > 0 {
            f64::from(total_project_complexity) / total_functions as f64
        } else {
            0.0
        };
        assert!(
            avg_complexity <= MAX_AVERAGE_COMPLEXITY,
            "Average data flow complexity too high: {avg_complexity:.2} (max allowed: {MAX_AVERAGE_COMPLEXITY:.2}). \
             Consider refactoring functions with complex data dependencies."
        );

        // OPTION 2: FILE-LEVEL - Prevent individual files from becoming too complex
        // Recommended: No single file should have average complexity > 40
        for (file_path, file_comp) in &file_complexities {
            if file_comp.avg_complexity > MAX_FILE_AVG_COMPLEXITY {
                eprintln!(
                    "Warning: File {} has high average data flow complexity: {:.2}",
                    file_path.display(),
                    file_comp.avg_complexity
                );
            }
        }

        // OPTION 3: MODERATE - Limit the number of very high complexity functions
        // Recommended: Allow 5-10% of functions to be very high complexity
        /*
        const MAX_VERY_HIGH_COMPLEXITY_FUNCTIONS: usize = 30;
        assert!(
            very_high <= MAX_VERY_HIGH_COMPLEXITY_FUNCTIONS,
            "Too many functions with very high data flow complexity (≥{}): {} (max allowed: {}). \
             Consider refactoring the most complex functions.",
            VERY_HIGH_COMPLEXITY,
            very_high,
            MAX_VERY_HIGH_COMPLEXITY_FUNCTIONS
        );
        */

        // OPTION 4: PROGRESSIVE - Prevent new extremely complex functions
        // This is more useful in CI/CD to prevent regression
        // Fail if any function exceeds a "hard limit" (e.g., 200+)
        /*
        const HARD_LIMIT_COMPLEXITY: u32 = 200;
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
                "Found {} function(s) exceeding hard data flow complexity limit (≥{}): {}. \
                 These MUST be refactored before merging.",
                extremely_complex.len(),
                HARD_LIMIT_COMPLEXITY,
                names.join(", ")
            );
        }
        */
    }
}
