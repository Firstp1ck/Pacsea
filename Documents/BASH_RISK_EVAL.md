# Bash Script Security Risk Evaluation Form

## Risk Assessment Methodology

This form evaluates bash scripts for malicious code patterns. Each section assigns points based on detected suspicious patterns. The final risk score ranges from **0% (safe) to 100% (highly malicious)**.

**Scoring Guide:**
- **0-20%:** Low Risk - Few or no suspicious patterns, likely safe
- **21-40%:** Medium-Low Risk - Some suspicious patterns present, review recommended
- **41-60%:** Medium Risk - Multiple suspicious patterns, caution advised
- **61-80%:** Medium-High Risk - Significant malicious indicators, do not execute
- **81-100%:** Critical Risk - Extensive malicious patterns, definitely malicious

---

## Section 1: Reconnaissance & Information Gathering
**Base Points: 0-10**

| Pattern Detected | Points | Found | Notes |
|------------------|--------|-------|-------|
| System info extraction (`whoami`, `uname`, `hostname`, `id`, `groups`) | 2 | ☐ | |
| User/privilege enumeration (`cat /etc/passwd`, `sudo -l`, `crontab -l`) | 3 | ☐ | |
| Network reconnaissance (`nmap`, `netstat`, `ss`, `ifconfig`, `arp`) | 3 | ☐ | |
| Cloud metadata access (`curl/wget 169.254.169.254`) | 4 | ☐ | |
| Sensitive file searching (`find`, `grep` for passwords/keys/secrets) | 3 | ☐ | |

**Section 1 Total Points: _____ / 10**

---

## Section 2: Privilege Escalation
**Base Points: 0-15**

| Pattern Detected | Points | Found | Notes |
|------------------|--------|-------|-------|
| Permission modification commands (`chmod`, `setfacl`) | 3 | ☐ | |
| Sudo manipulation (`sudo -l`, sudoers modification) | 4 | ☐ | |
| Critical file modification (`/etc/sudoers`, `/etc/shadow`) | 5 | ☐ | |
| Library preload hijacking (`/etc/ld.so.preload`, `chattr`) | 4 | ☐ | |
| World-readable/writable permission grants (`chmod 777`, `chmod 644 /etc/*`) | 5 | ☐ | |

**Section 2 Total Points: _____ / 15**

---

## Section 3: Malicious Code Execution & Injection
**Base Points: 0-20**

| Pattern Detected | Points | Found | Notes |
|------------------|--------|-------|-------|
| Backtick/dollar-paren command substitution with external input | 3 | ☐ | |
| `eval` with untrusted input | 5 | ☐ | |
| Reverse shell patterns (`/dev/tcp`, interactive bash) | 5 | ☐ | |
| Network file descriptor operations (`exec XX<>/dev/tcp`) | 4 | ☐ | |
| Base64/hex/octal encoded command execution | 5 | ☐ | |
| Dynamic variable concatenation to assemble commands | 4 | ☐ | |
| Payload download & execution in pipeline (`wget/curl | bash`) | 5 | ☐ | |

**Section 3 Total Points: _____ / 20**

---

## Section 4: Data Theft & Exfiltration
**Base Points: 0-12**

| Pattern Detected | Points | Found | Notes |
|------------------|--------|-------|-------|
| File compression (`tar`, `zip`) of sensitive directories | 3 | ☐ | |
| Data transfer commands (`scp`, `curl` upload, `wget`, `nc`) | 3 | ☐ | |
| Credential extraction (`cat /etc/shadow`, `~/.ssh/id_rsa`, `~/.bash_history`) | 4 | ☐ | |
| Environment variable searching for credentials (`env \| grep -i pass`) | 2 | ☐ | |

**Section 4 Total Points: _____ / 12**

---

## Section 5: Trail Covering & Log Manipulation
**Base Points: 0-15**

| Pattern Detected | Points | Found | Notes |
|------------------|--------|-------|-------|
| Bash history erasure (`HISTFILESIZE=0`, `history -c`, `rm ~/.bash_history`) | 4 | ☐ | |
| Syslog/audit interruption (`service auditd stop`, `rsyslog stop`) | 4 | ☐ | |
| Log file truncation (`echo "" > /var/log/*`) | 3 | ☐ | |
| File attribute manipulation to hide changes (`chattr`) | 3 | ☐ | |

**Section 5 Total Points: _____ / 15**

---

## Section 6: Dangerous System Operations
**Base Points: 0-18**

| Pattern Detected | Points | Found | Notes |
|------------------|--------|-------|-------|
| Destructive filesystem commands (`rm -rf /`, `dd if=/dev/zero`) | 5 | ☐ | |
| Fork bomb patterns (`:() { :|:& };:` or similar) | 5 | ☐ | |
| Resource exhaustion (`ulimit -u unlimited`, `yes > /dev/null &`) | 4 | ☐ | |
| Hard drive overwrite operations | 5 | ☐ | |

**Section 6 Total Points: _____ / 18**

---

## Section 7: Persistence Mechanisms
**Base Points: 0-15**

| Pattern Detected | Points | Found | Notes |
|------------------|--------|-------|-------|
| Cron job insertion (`crontab`, `/etc/cron.*`) | 4 | ☐ | |
| Shell config modification (`~/.bashrc`, `~/.bash_profile`, `/etc/profile`) | 3 | ☐ | |
| SSH backdoor key insertion (`authorized_keys` modification) | 4 | ☐ | |
| Malicious library preload installation | 4 | ☐ | |

**Section 7 Total Points: _____ / 15**

---

## Section 8: Network Communication & C&C
**Base Points: 0-12**

| Pattern Detected | Points | Found | Notes |
|------------------|--------|-------|-------|
| Proxy configuration for C&C (`http_proxy`, `https_proxy`, `ALL_PROXY`) | 3 | ☐ | |
| C&C process/port hunting (`netstat`, `grep` for specific IPs/ports) | 3 | ☐ | |
| Process termination hiding involvement (`kill -9` combined with grep) | 3 | ☐ | |

**Section 8 Total Points: _____ / 12**

---

## Section 9: Anti-Analysis & Evasion
**Base Points: 0-10**

| Pattern Detected | Points | Found | Notes |
|------------------|--------|-------|-------|
| Script compilation to binary (`shc`) or Bashfuscator usage | 3 | ☐ | |
| Multi-layer encoding (base64 → base32 → hex) | 3 | ☐ | |
| Minified/single-line code | 2 | ☐ | |
| Shellshock patterns in environment variables | 3 | ☐ | |
| Binary masquerading (executable as image file) | 2 | ☐ | |

**Section 9 Total Points: _____ / 10**

---

## Section 10: File Descriptors & Socket Operations
**Base Points: 0-8**

| Pattern Detected | Points | Found | Notes |
|------------------|--------|-------|-------|
| Unusual socket file descriptor assignments (`exec 3<>/dev/tcp`) | 3 | ☐ | |
| Custom redirection for hidden communication | 3 | ☐ | |
| Direct I/O through file descriptors | 2 | ☐ | |

**Section 10 Total Points: _____ / 8**

---

## Section 11: Suspicious Code Patterns
**Base Points: 0-10**

| Pattern Detected | Points | Found | Notes |
|------------------|--------|-------|-------|
| `eval` combined with `read` + user input | 4 | ☐ | |
| Download + pipe to interpreter combination | 4 | ☐ | |
| Multiple obfuscation techniques chained together | 3 | ☐ | |
| Unused functions or variables (potential obfuscation) | 2 | ☐ | |
| Excessive `tr`/`sed`/`awk` for string manipulation | 2 | ☐ | |

**Section 11 Total Points: _____ / 10**

---

## Severity Multiplier for Dangerous Combinations

**Identify if multiple patterns from different categories appear together:**

| Combination | Multiplier | Found | Notes |
|-------------|-----------|-------|-------|
| Reconnaissance + Data Theft + Trail Covering | ×1.3 | ☐ | Typical attack chain |
| Download/Execute + Persistence + C&C | ×1.3 | ☐ | Backdoor installation |
| Privilege Escalation + Root-level Persistence | ×1.4 | ☐ | System compromise |
| Fork Bomb + History Clearing | ×1.2 | ☐ | Sabotage pattern |
| Multiple Encoding Layers + Evasion | ×1.3 | ☐ | Sophisticated malware |

**Multiplier Applied:** _____ (default: 1.0)

---

## FINAL RISK SCORE CALCULATION

| Category | Points | Max | Percentage |
|----------|--------|-----|-----------|
| Section 1 (Reconnaissance) | _____ | 10 | ____% |
| Section 2 (Privilege Escalation) | _____ | 15 | ____% |
| Section 3 (Code Execution) | _____ | 20 | ____% |
| Section 4 (Data Theft) | _____ | 12 | ____% |
| Section 5 (Trail Covering) | _____ | 15 | ____% |
| Section 6 (Dangerous Operations) | _____ | 18 | ____% |
| Section 7 (Persistence) | _____ | 15 | ____% |
| Section 8 (C&C Communication) | _____ | 12 | ____% |
| Section 9 (Anti-Analysis) | _____ | 10 | ____% |
| Section 10 (File Descriptors) | _____ | 8 | ____% |
| Section 11 (Code Patterns) | _____ | 10 | ____% |
| **SUBTOTAL** | **_____** | **135** | |

**Severity Multiplier:** _____ × Subtotal = **_____**

**FINAL RISK SCORE: _____ / 135**

---

## Risk Classification

**Overall Risk Level:**

- ☐ **0-20% - LOW RISK**
  - Verdict: Likely safe, minimal suspicious patterns
  - Recommendation: Can review and execute with low concern

- ☐ **21-40% - MEDIUM-LOW RISK**
  - Verdict: Some suspicious patterns present
  - Recommendation: Manual code review recommended before execution

- ☐ **41-60% - MEDIUM RISK**
  - Verdict: Multiple concerning patterns detected
  - Recommendation: Detailed analysis required, consider sandboxing before execution

- ☐ **61-80% - MEDIUM-HIGH RISK**
  - Verdict: Significant malicious indicators present
  - Recommendation: Do not execute, escalate for investigation

- ☐ **81-100% - CRITICAL RISK**
  - Verdict: Extensive malicious patterns indicate probable malware
  - Recommendation: Do not execute, treat as active threat, investigate origin

## Appendix: Quick Reference

### Red Flags (5+ points immediately):
- Any reverse shell pattern
- Payload download + pipe to bash
- Fork bombs
- rm -rf / variants
- Cron + reverse shell combination

### Orange Flags (2-3 points):
- eval with user input
- History clearing alone
- Single privilege escalation attempt
- Reconnaissance commands

### Automated Analysis Tools:
- **ShellCheck:** `shellcheck script.sh` (detects syntax/security issues)
- **unshell:** Deobfuscate multi-layer encoded scripts
- **bash -x:** Trace execution to reveal obfuscated code
- **Falco:** Runtime behavior detection
