#! /usr/bin/env bash
# test_archlinux_endpoints.sh

echo "Testing archlinux.org endpoints..."

# Timeout configuration (in seconds)
CONNECT_TIMEOUT=10
MAX_TIME=30

# Initialize evaluation arrays
declare -a test_names
declare -a http_statuses
declare -a response_times
declare -a parse_results
declare -a issues

# Test 1: News Feed
echo -e "\n1. News Feed:"
NEWS_BODY=$(curl -s --connect-timeout $CONNECT_TIMEOUT --max-time $MAX_TIME "https://archlinux.org/feeds/news/")
NEWS_CURL_EXIT=$?
NEWS_STATS=$(curl -s --connect-timeout $CONNECT_TIMEOUT --max-time $MAX_TIME -w "%{http_code}|%{time_total}" \
  "https://archlinux.org/feeds/news/" -o /dev/null)
NEWS_STATS_EXIT=$?
NEWS_HTTP=$(echo "$NEWS_STATS" | cut -d'|' -f1)
NEWS_TIME=$(echo "$NEWS_STATS" | cut -d'|' -f2)

# Exit on timeout or connection failure
# Exit codes: 28=timeout, 6=couldn't resolve host, 7=failed to connect
if [ $NEWS_CURL_EXIT -eq 28 ] || [ $NEWS_CURL_EXIT -eq 6 ] || [ $NEWS_CURL_EXIT -eq 7 ] || \
   [ $NEWS_STATS_EXIT -eq 28 ] || [ $NEWS_STATS_EXIT -eq 6 ] || [ $NEWS_STATS_EXIT -eq 7 ] || \
   [ -z "$NEWS_HTTP" ] || [ "$NEWS_HTTP" = "000" ]; then
    echo "❌ Connection timeout or failure after ${CONNECT_TIMEOUT}s"
    echo "Exiting script."
    exit 1
fi

echo "$NEWS_BODY" | head -20
echo "HTTP Status: $NEWS_HTTP | Time: ${NEWS_TIME}s"

test_names+=("News Feed")
http_statuses+=("$NEWS_HTTP")
response_times+=("$NEWS_TIME")
if echo "$NEWS_BODY" | grep -q "<rss"; then
    parse_results+=("OK")
else
    parse_results+=("FAIL")
    issues+=("News Feed: Invalid RSS format")
fi

# Test 2: Package JSON (pacman)
echo -e "\n2. Package JSON (pacman):"
PKG_BODY=$(curl -s --connect-timeout $CONNECT_TIMEOUT --max-time $MAX_TIME "https://archlinux.org/packages/core/x86_64/pacman/json/")
PKG_CURL_EXIT=$?
PKG_STATS=$(curl -s --connect-timeout $CONNECT_TIMEOUT --max-time $MAX_TIME -w "%{http_code}|%{time_total}" \
  "https://archlinux.org/packages/core/x86_64/pacman/json/" -o /dev/null)
PKG_STATS_EXIT=$?
PKG_HTTP=$(echo "$PKG_STATS" | cut -d'|' -f1)
PKG_TIME=$(echo "$PKG_STATS" | cut -d'|' -f2)

# Exit on timeout or connection failure
# Exit codes: 28=timeout, 6=couldn't resolve host, 7=failed to connect
if [ $PKG_CURL_EXIT -eq 28 ] || [ $PKG_CURL_EXIT -eq 6 ] || [ $PKG_CURL_EXIT -eq 7 ] || \
   [ $PKG_STATS_EXIT -eq 28 ] || [ $PKG_STATS_EXIT -eq 6 ] || [ $PKG_STATS_EXIT -eq 7 ] || \
   [ -z "$PKG_HTTP" ] || [ "$PKG_HTTP" = "000" ]; then
    echo "❌ Connection timeout or failure after ${CONNECT_TIMEOUT}s"
    echo "Exiting script."
    exit 1
fi

# Try to extract name and version (handle both .pkg.name and .pkgname formats)
PKG_NAME=$(echo "$PKG_BODY" | jq -r '.pkg.pkgname // .pkg.name // .pkgname // "unknown"' 2>/dev/null)
PKG_VER=$(echo "$PKG_BODY" | jq -r '.pkg.pkgver // .pkg.version // .pkgver // "unknown"' 2>/dev/null)
echo "$PKG_NAME $PKG_VER"
echo "HTTP Status: $PKG_HTTP | Time: ${PKG_TIME}s"

test_names+=("Package JSON")
http_statuses+=("$PKG_HTTP")
response_times+=("$PKG_TIME")
# Check if JSON is valid and has either pkgname or .pkg.pkgname or .pkg.name
if echo "$PKG_BODY" | jq -e '.pkgname // .pkg.pkgname // .pkg.name' >/dev/null 2>&1; then
    parse_results+=("OK")
else
    parse_results+=("FAIL")
    issues+=("Package JSON: Invalid JSON or missing package name field")
fi

# Test 3: Package Search
echo -e "\n3. Package Search:"
SEARCH_BODY=$(curl -s --connect-timeout $CONNECT_TIMEOUT --max-time $MAX_TIME "https://archlinux.org/packages/search/json/?repo=core&arch=x86_64&limit=5&page=1")
SEARCH_CURL_EXIT=$?
SEARCH_STATS=$(curl -s --connect-timeout $CONNECT_TIMEOUT --max-time $MAX_TIME -w "%{http_code}|%{time_total}" \
  "https://archlinux.org/packages/search/json/?repo=core&arch=x86_64&limit=5&page=1" -o /dev/null)
SEARCH_STATS_EXIT=$?
SEARCH_HTTP=$(echo "$SEARCH_STATS" | cut -d'|' -f1)
SEARCH_TIME=$(echo "$SEARCH_STATS" | cut -d'|' -f2)

# Exit on timeout or connection failure
# Exit codes: 28=timeout, 6=couldn't resolve host, 7=failed to connect
if [ $SEARCH_CURL_EXIT -eq 28 ] || [ $SEARCH_CURL_EXIT -eq 6 ] || [ $SEARCH_CURL_EXIT -eq 7 ] || \
   [ $SEARCH_STATS_EXIT -eq 28 ] || [ $SEARCH_STATS_EXIT -eq 6 ] || [ $SEARCH_STATS_EXIT -eq 7 ] || \
   [ -z "$SEARCH_HTTP" ] || [ "$SEARCH_HTTP" = "000" ]; then
    echo "❌ Connection timeout or failure after ${CONNECT_TIMEOUT}s"
    echo "Exiting script."
    exit 1
fi

echo "$SEARCH_BODY" | jq '.results | length' 2>/dev/null
echo "HTTP Status: $SEARCH_HTTP | Time: ${SEARCH_TIME}s"

test_names+=("Package Search")
http_statuses+=("$SEARCH_HTTP")
response_times+=("$SEARCH_TIME")
if echo "$SEARCH_BODY" | jq -e '.results' >/dev/null 2>&1; then
    parse_results+=("OK")
else
    parse_results+=("FAIL")
    issues+=("Package Search: Invalid JSON or missing .results")
fi

# Test 4: Mirror Status
echo -e "\n4. Mirror Status:"
MIRROR_BODY=$(curl -s --connect-timeout $CONNECT_TIMEOUT --max-time $MAX_TIME "https://archlinux.org/mirrors/status/json/")
MIRROR_CURL_EXIT=$?
MIRROR_STATS=$(curl -s --connect-timeout $CONNECT_TIMEOUT --max-time $MAX_TIME -w "%{http_code}|%{time_total}" \
  "https://archlinux.org/mirrors/status/json/" -o /dev/null)
MIRROR_STATS_EXIT=$?
MIRROR_HTTP=$(echo "$MIRROR_STATS" | cut -d'|' -f1)
MIRROR_TIME=$(echo "$MIRROR_STATS" | cut -d'|' -f2)

# Exit on timeout or connection failure
# Exit codes: 28=timeout, 6=couldn't resolve host, 7=failed to connect
if [ $MIRROR_CURL_EXIT -eq 28 ] || [ $MIRROR_CURL_EXIT -eq 6 ] || [ $MIRROR_CURL_EXIT -eq 7 ] || \
   [ $MIRROR_STATS_EXIT -eq 28 ] || [ $MIRROR_STATS_EXIT -eq 6 ] || [ $MIRROR_STATS_EXIT -eq 7 ] || \
   [ -z "$MIRROR_HTTP" ] || [ "$MIRROR_HTTP" = "000" ]; then
    echo "❌ Connection timeout or failure after ${CONNECT_TIMEOUT}s"
    echo "Exiting script."
    exit 1
fi

echo "$MIRROR_BODY" | jq 'keys | length' 2>/dev/null
echo "HTTP Status: $MIRROR_HTTP | Time: ${MIRROR_TIME}s"

test_names+=("Mirror Status")
http_statuses+=("$MIRROR_HTTP")
response_times+=("$MIRROR_TIME")
if echo "$MIRROR_BODY" | jq -e 'keys' >/dev/null 2>&1; then
    parse_results+=("OK")
else
    parse_results+=("FAIL")
    issues+=("Mirror Status: Invalid JSON or missing keys")
fi

# Evaluation
echo -e "\n""$(printf '=%.0s' {1..60})"
echo "EVALUATION SUMMARY"
echo -e "$(printf '=%.0s' {1..60})"

all_good=true
max_time=5.0

for i in "${!test_names[@]}"; do
    name="${test_names[$i]}"
    http="${http_statuses[$i]}"
    time="${response_times[$i]}"
    parse="${parse_results[$i]}"
    
    # Check HTTP status
    if [ -z "$http" ] || [ "$http" = "000" ] || [ "$http" != "200" ]; then
        all_good=false
        if [ -z "$http" ] || [ "$http" = "000" ]; then
            issues+=("$name: Connection failed or timed out")
        else
            issues+=("$name: HTTP status $http (expected 200)")
        fi
    fi
    
    # Check response time (using awk for float comparison)
    # awk returns 0 (success) if condition is true, 1 if false
    if [ -z "$time" ]; then
        all_good=false
        issues+=("$name: Failed to measure response time")
    elif awk "BEGIN {if ($time > $max_time) exit 0; else exit 1}"; then
        all_good=false
        issues+=("$name: Slow response time ${time}s (threshold: ${max_time}s)")
    fi
    
    # Check parse result
    if [ "$parse" != "OK" ]; then
        all_good=false
    fi
done

if [ "$all_good" = true ]; then
    echo -e "\n✅ ALL TESTS PASSED"
    echo "All endpoints are responding correctly:"
    for i in "${!test_names[@]}"; do
        echo "  • ${test_names[$i]}: HTTP ${http_statuses[$i]}, ${response_times[$i]}s, Parse ${parse_results[$i]}"
    done
else
    echo -e "\n❌ ISSUES DETECTED"
    echo ""
    echo "Problems found:"
    for issue in "${issues[@]}"; do
        echo "  • $issue"
    done
    echo ""
    echo "Test details:"
    for i in "${!test_names[@]}"; do
        status_icon="✅"
        time_check=$(awk "BEGIN {if (${response_times[$i]} > $max_time) exit 0; else exit 1}" && echo "slow" || echo "ok")
        if [ "${http_statuses[$i]}" != "200" ] || [ "${parse_results[$i]}" != "OK" ] || [ "$time_check" = "slow" ]; then
            status_icon="❌"
        fi
        echo "  $status_icon ${test_names[$i]}: HTTP ${http_statuses[$i]}, ${response_times[$i]}s, Parse ${parse_results[$i]}"
    done
fi

echo -e "$(printf '=%.0s' {1..60})"