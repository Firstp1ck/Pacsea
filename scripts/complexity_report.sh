#!/usr/bin/env bash

cargo test complexity -- --nocapture 2>&1 | grep -vE "(^running|^test result:|^test tests::|Finished.*test.*profile|Running unittests|Running tests/)" | sed '/^$/N;/^\n$/d' | awk '
  /^=== Cyclomatic Complexity Report ===/ { section="cyclomatic"; delete top3_cyclomatic; count_cyc=0 }
  /^=== Data Flow Complexity Report/ { section="dataflow"; delete top3_dataflow; count_df=0 }
  /^=== Top 10 Most Complex Functions ===/ && section=="cyclomatic" { in_top10=1; count_cyc=0; next }
  /^=== Top 10 Most Complex Functions ===/ && section=="dataflow" { in_top10=1; count_df=0; next }
  in_top10 && /^[0-9]+\./ && section=="cyclomatic" && count_cyc < 3 { top3_cyclomatic[++count_cyc]=$0 }
  in_top10 && /^[0-9]+\./ && section=="dataflow" && count_df < 3 { top3_dataflow[++count_df]=$0 }
  in_top10 && /^===/ { in_top10=0 }
  { lines[++line_count]=$0 }
  END {
    for (i=1; i<=line_count; i++) print lines[i]
    print ""
    print "=== Evaluation Summary: Top 3 Highest Scores ==="
    print ""
    print "Cyclomatic Complexity - Top 3 Functions:"
    for (i=1; i<=3; i++) if (top3_cyclomatic[i]) print top3_cyclomatic[i]
    print ""
    print "Data Flow Complexity - Top 3 Functions:"
    for (i=1; i<=3; i++) if (top3_dataflow[i]) print top3_dataflow[i]
  }
' | tee complexity_report.txt