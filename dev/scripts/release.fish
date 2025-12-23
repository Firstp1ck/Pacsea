#!/usr/bin/env fish
# release.fish - Automated version release script for Pacsea
#
# What: Automates the entire release workflow including version bumps,
#       documentation, building, and AUR publishing.
#
# Usage:
#   ./release.fish [--dry-run] [version]
#
# Options:
#   --dry-run    Preview all changes without executing them
#   version      New version (e.g., 0.6.2). If not provided, will prompt.
#
# Details:
#   This script guides through the complete release process:
#   1. Version update in Cargo.toml
#   2. Documentation (release notes, announcements, README, wiki)
#   3. PKGBUILD updates
#   4. Build and GitHub release
#   5. AUR publishing

# ============================================================================
# Configuration
# ============================================================================

set -g PACSEA_DIR (realpath (dirname (status filename))/../..)
set -g AUR_BIN_DIR "$HOME/aur-packages/pacsea-bin"
set -g AUR_GIT_DIR "$HOME/aur-packages/pacsea-git"
set -g WIKI_DIR "$HOME/Dokumente/GitHub/Pacsea.wiki"
set -g DRY_RUN false

# Colors for output - use functions to avoid variable interpolation issues
function _red; set_color red; end
function _green; set_color green; end
function _yellow; set_color yellow; end
function _blue; set_color blue; end
function _cyan; set_color cyan; end
function _magenta; set_color magenta; end
function _reset; set_color normal; end
function _bold; set_color --bold; end
function _bold_cyan; set_color --bold cyan; end
function _bold_green; set_color --bold green; end

# ============================================================================
# Helper Functions
# ============================================================================

function log_info
    _blue; echo -n "[INFO] "; _reset; echo $argv
end

function log_success
    _green; echo -n "[SUCCESS] "; _reset; echo $argv
end

function log_warn
    _yellow; echo -n "[WARN] "; _reset; echo $argv
end

function log_error
    _red; echo -n "[ERROR] "; _reset; echo $argv
end

function log_step
    echo
    _magenta; echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"; _reset
    _bold_cyan; echo "  STEP: $argv"; _reset
    _magenta; echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"; _reset
end

function log_phase
    echo
    _bold_green; echo "════════════════════════════════════════════════════════════════════════"; _reset
    _bold_green; echo "  PHASE: $argv"; _reset
    _bold_green; echo "════════════════════════════════════════════════════════════════════════"; _reset
end

function dry_run_cmd
    if test "$DRY_RUN" = true
        _yellow; echo -n "[DRY-RUN] Would execute: "; _reset; echo $argv
        return 0
    else
        eval $argv
        return $status
    end
end

function confirm_continue
    set -l msg $argv[1]
    if test -z "$msg"
        set msg "Continue?"
    end
    
    while true
        _cyan; echo -n "$msg [Y/n]: "; _reset
        read response
        switch (string lower $response)
            case '' y yes
                return 0
            case n no
                return 1
            case '*'
                echo "Please answer y or n"
        end
    end
end

function wait_for_user
    set -l msg $argv[1]
    if test -z "$msg"
        set msg "Press Enter to continue..."
    end
    _cyan; echo -n $msg; _reset
    read
end

function validate_semver
    set -l ver_str $argv[1]
    if string match -r '^[0-9]+\.[0-9]+\.[0-9]+$' $ver_str
        return 0
    else
        return 1
    end
end

function get_current_version
    grep -m1 '^version = ' "$PACSEA_DIR/Cargo.toml" | sed 's/version = "\(.*\)"/\1/'
end

function is_prerelease_version
    # Returns 0 (true) if version is < 1.0.0 (prerelease)
    # Returns 1 (false) if version is >= 1.0.0 (stable)
    set -l ver_str $argv[1]
    set -l major (string split '.' $ver_str)[1]
    if test "$major" -lt 1 2>/dev/null
        return 0
    else
        return 1
    end
end

function is_major_or_minor_change
    # Returns 0 (true) if major or minor version changed (not just patch)
    # Returns 1 (false) if only patch version changed
    set -l old_ver $argv[1]
    set -l new_ver $argv[2]
    
    set -l old_parts (string split '.' $old_ver)
    set -l new_parts (string split '.' $new_ver)
    
    set -l old_major $old_parts[1]
    set -l old_minor $old_parts[2]
    set -l new_major $new_parts[1]
    set -l new_minor $new_parts[2]
    
    # Check if major or minor changed
    if test "$old_major" != "$new_major"
        return 0
    else if test "$old_minor" != "$new_minor"
        return 0
    else
        return 1
    end
end

# ============================================================================
# Phase 1: Version Update
# ============================================================================

function phase1_version_update
    set -l new_ver $argv[1]
    
    log_phase "1. Version Update"
    
    set -l current_ver (get_current_version)
    _blue; echo -n "[INFO] "; _reset; echo -n "Current version: "; _bold; echo $current_ver; _reset
    _blue; echo -n "[INFO] "; _reset; echo -n "New version: "; _bold; echo $new_ver; _reset
    
    # Step 1.1: Update Cargo.toml
    log_step "Updating Cargo.toml"
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would update version in Cargo.toml from $current_ver to $new_ver"
    else
        sed -i "s/^version = \"$current_ver\"/version = \"$new_ver\"/" "$PACSEA_DIR/Cargo.toml"
        if test $status -eq 0
            log_success "Updated Cargo.toml"
        else
            log_error "Failed to update Cargo.toml"
            return 1
        end
    end
    
    # Step 1.2: Run cargo check to update Cargo.lock
    log_step "Updating Cargo.lock"
    
    cd "$PACSEA_DIR"
    dry_run_cmd "cargo check"
    if test $status -eq 0
        log_success "Cargo.lock updated"
    else
        log_error "cargo check failed"
        return 1
    end
    
    return 0
end

# ============================================================================
# Phase 2: Documentation
# ============================================================================

function phase2_documentation
    set -l new_ver $argv[1]
    set -l old_ver $argv[2]
    
    log_phase "2. Documentation"
    
    # Step 2.1: Generate release notes with Cursor
    log_step "Generate Release Notes"
    _blue; echo -n "[INFO] "; _reset; echo -n "Please run: "; _bold; echo "/release-new $new_ver"; _reset
    
    set -l release_file "$PACSEA_DIR/Documents/RELEASE_v$new_ver.md"
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would wait for release notes generation"
    else
        wait_for_user "After completing /release-new in Cursor, press Enter..."
        
        # Verify release file was created
        if not test -f "$release_file"
            log_warn "Release file not found at: $release_file"
            if not confirm_continue "Continue anyway?"
                return 1
            end
        else
            log_success "Release file created: $release_file"
        end
    end
    
    # Step 2.2: Update CHANGELOG.md
    update_changelog "$new_ver"
    
    # Step 2.3: Auto-generate announcement from release file
    log_step "Auto-generate Announcement"
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would generate announcement from release file"
    else
        if test -f "$release_file"
            generate_announcement "$release_file" "$new_ver"
        else
            log_warn "Skipping announcement generation (no release file)"
        end
    end
    
    # Step 2.4: Run update_version_announcement.py
    log_step "Update Version Announcement in Code"
    
    set -l announcement_file "$PACSEA_DIR/dev/ANNOUNCEMENTS/version_announcement_content.md"
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would run update_version_announcement.py"
    else
        if test -f "$announcement_file"
            set -l announcement_content (cat "$announcement_file")
            python3 "$PACSEA_DIR/dev/scripts/update_version_announcement.py" \
                "$new_ver" \
                "Version $new_ver" \
                --file "$announcement_file"
            
            if test $status -eq 0
                log_success "Announcement updated in code"
            else
                log_error "Failed to update announcement"
                if not confirm_continue "Continue anyway?"
                    return 1
                end
            end
        else
            log_warn "Announcement file not found, skipping..."
        end
    end
    
    # Step 2.5: README update with Cursor
    log_step "Update README"
    _blue; echo -n "[INFO] "; _reset; echo -n "Please run: "; _bold; echo "/readme-update"; _reset
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would wait for README update"
    else
        wait_for_user "After completing /readme-update in Cursor, press Enter..."
        log_success "README update complete"
    end
    
    # Step 2.6: Wiki update with Cursor
    log_step "Update Wiki"
    _blue; echo -n "[INFO] "; _reset; echo -n "Please run: "; _bold; echo "/wiki-update"; _reset
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would wait for wiki update"
    else
        wait_for_user "After completing /wiki-update in Cursor, press Enter..."
        log_success "Wiki update complete"
    end
    
    # Step 2.7: Update SECURITY.md if major/minor version changed
    if is_major_or_minor_change "$old_ver" "$new_ver"
        log_step "Update SECURITY.md"
        update_security_md "$new_ver"
    else
        log_info "Skipping SECURITY.md update (patch release only)"
    end
    
    return 0
end

function generate_announcement
    set -l release_file $argv[1]
    set -l ver $argv[2]
    set -l output_file "$PACSEA_DIR/dev/ANNOUNCEMENTS/version_announcement_content.md"
    
    log_info "Parsing release file for announcement..."
    
    # Collect all bullet points from the "What's New" section
    set -l all_items
    set -l in_whats_new false
    set -l current_subsection ""
    
    # Parse the release file line by line
    while read -l line
        # Detect "## What's New" section start
        if string match -r '^##\s+What' "$line"
            set in_whats_new true
            continue
        end
        
        # Detect end of "What's New" section (another ## header)
        if test "$in_whats_new" = true
            if string match -r '^##\s+' "$line"
                # Another major section - stop parsing
                set in_whats_new false
                continue
            end
        end
        
        # Inside What's New section
        if test "$in_whats_new" = true
            # Detect subsection headers (### ...)
            if string match -r '^###' "$line"
                # Extract subsection name (strip ### and emoji)
                set current_subsection (string replace -r '^###\s*[^\s]*\s*' '' "$line" | string trim)
                continue
            end
            
            # Extract bullet points
            if string match -r '^[-*]\s+' "$line"
                set -l item (string replace -r '^[-*]\s+' '' "$line")
                # Truncate long items
                if test (string length "$item") -gt 80
                    set item (string sub -l 77 "$item")"..."
                end
                # Skip empty items
                if test -n "$item"
                    set -a all_items "$item"
                end
            end
        end
    end < "$release_file"
    
    # Build the announcement content
    set -l tmp_output (mktemp)
    echo "## What's New" > "$tmp_output"
    echo "" >> "$tmp_output"
    
    # Add items (limit to 5 total)
    set -l count 0
    for item in $all_items
        if test $count -lt 5
            echo "- $item" >> "$tmp_output"
            set count (math $count + 1)
        end
    end
    echo "" >> "$tmp_output"
    
    # Check character count (max 800)
    set -l char_count (wc -c < "$tmp_output" | string trim)
    if test $char_count -gt 800
        log_warn "Announcement exceeds 800 chars ($char_count), truncating..."
    end
    
    # Move to output file
    mv "$tmp_output" "$output_file"
    log_success "Generated announcement ($char_count chars) at: $output_file"
    
    # Show preview
    echo
    _cyan; echo "--- Announcement Preview ---"; _reset
    cat "$output_file"
    _cyan; echo "--- End Preview ---"; _reset
    echo
    
    if not confirm_continue "Accept this announcement?"
        log_info "Please edit the file manually..."
        wait_for_user "Press Enter after editing $output_file..."
    end
end

# ============================================================================
# Phase 3: PKGBUILD Updates
# ============================================================================

function phase3_pkgbuild_updates
    set -l new_ver $argv[1]
    
    log_phase "3. PKGBUILD Updates"
    
    # Step 3.1: Update pacsea-bin PKGBUILD
    log_step "Update pacsea-bin PKGBUILD"
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would update pkgver to $new_ver in $AUR_BIN_DIR/PKGBUILD"
        log_info "[DRY-RUN] Would reset pkgrel to 1"
    else
        if test -f "$AUR_BIN_DIR/PKGBUILD"
            # Update pkgver
            sed -i "s/^pkgver=.*/pkgver=$new_ver/" "$AUR_BIN_DIR/PKGBUILD"
            # Reset pkgrel to 1
            sed -i "s/^pkgrel=.*/pkgrel=1/" "$AUR_BIN_DIR/PKGBUILD"
            log_success "Updated $AUR_BIN_DIR/PKGBUILD"
        else
            log_error "PKGBUILD not found at $AUR_BIN_DIR/PKGBUILD"
            return 1
        end
    end
    
    # Step 3.2: Update pacsea-git PKGBUILD
    log_step "Update pacsea-git PKGBUILD"
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would update pkgver to $new_ver in $AUR_GIT_DIR/PKGBUILD"
        log_info "[DRY-RUN] Would reset pkgrel to 1"
        log_info "[DRY-RUN] Would remove git commit suffixes"
    else
        if test -f "$AUR_GIT_DIR/PKGBUILD"
            # Update pkgver (remove any git commit suffix like .r3.g6867376)
            sed -i "s/^pkgver=.*/pkgver=$new_ver/" "$AUR_GIT_DIR/PKGBUILD"
            # Reset pkgrel to 1
            sed -i "s/^pkgrel=.*/pkgrel=1/" "$AUR_GIT_DIR/PKGBUILD"
            log_success "Updated $AUR_GIT_DIR/PKGBUILD"
        else
            log_error "PKGBUILD not found at $AUR_GIT_DIR/PKGBUILD"
            return 1
        end
    end
    
    log_success "PKGBUILD updates complete"
    return 0
end

# ============================================================================
# Phase 4: Build and Release
# ============================================================================

function phase4_build_release
    set -l new_ver $argv[1]
    
    log_phase "4. Build and Release"
    
    cd "$PACSEA_DIR"
    
    # Step 4.1: Run cargo-dev (tests/checks)
    log_step "Running cargo-dev (tests and checks)"
    
    dry_run_cmd "cargo-dev"
    if test $status -ne 0
        log_error "cargo-dev failed"
        if not confirm_continue "Continue despite cargo-dev failure?"
            return 1
        end
    else
        log_success "cargo-dev passed"
    end
    
    # Step 4.2: Build release binary
    log_step "Building release binary"
    
    dry_run_cmd "cargo build --release"
    if test $status -ne 0
        log_error "cargo build --release failed"
        return 1
    end
    log_success "Release binary built"
    
    # Step 4.3: Commit and push all changes
    log_step "Committing and pushing changes"
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would commit all changes with message: Release v$new_ver"
        log_info "[DRY-RUN] Would push to origin"
    else
        cd "$PACSEA_DIR"
        git add -A
        git commit -m "Release v$new_ver"
        if test $status -eq 0
            log_success "Changes committed"
        else
            log_warn "Nothing to commit or commit failed"
        end
        
        git push origin
        if test $status -eq 0
            log_success "Changes pushed to origin"
        else
            log_error "Failed to push changes"
            return 1
        end
    end
    
    # Step 4.4: Create git tag
    log_step "Creating git tag"
    
    set -l tag "v$new_ver"
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would create tag: $tag"
    else
        # Check if tag already exists
        if git tag -l | grep -q "^$tag\$"
            log_warn "Tag $tag already exists"
            if confirm_continue "Delete and recreate tag?"
                git tag -d "$tag"
                git push origin --delete "$tag" 2>/dev/null
            else
                log_info "Skipping tag creation"
            end
        end
        
        git tag "$tag"
        if test $status -eq 0
            log_success "Created tag: $tag"
        else
            log_error "Failed to create tag"
            return 1
        end
    end
    
    # Step 4.5: Push tag to GitHub
    log_step "Pushing tag to GitHub"
    
    dry_run_cmd "git push origin $tag"
    if test $status -eq 0
        log_success "Tag pushed to GitHub"
    else
        log_error "Failed to push tag"
        return 1
    end
    
    # Step 4.6: Create GitHub release (binary uploaded by GitHub Action)
    log_step "Creating GitHub Release"
    
    set -l release_file "$PACSEA_DIR/Documents/RELEASE_v$new_ver.md"
    
    # Determine if this is a prerelease (version < 1.0.0)
    set -l prerelease_flag ""
    if is_prerelease_version "$new_ver"
        set prerelease_flag "--prerelease"
        log_info "Version < 1.0.0: Creating as prerelease"
    else
        log_info "Version >= 1.0.0: Creating as stable release"
    end
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would create GitHub release $tag with notes from $release_file"
        log_info "[DRY-RUN] Binary will be uploaded by GitHub Action"
        if test -n "$prerelease_flag"
            log_info "[DRY-RUN] Release would be marked as prerelease"
        end
    else
        if test -f "$release_file"
            # Create release with notes (binary uploaded by GitHub Action)
            if test -n "$prerelease_flag"
                gh release create "$tag" \
                    --title "v$new_ver" \
                    --prerelease \
                    --notes-file "$release_file"
            else
                gh release create "$tag" \
                    --title "v$new_ver" \
                    --notes-file "$release_file"
            end
            
            if test $status -eq 0
                log_success "GitHub release created (binary will be uploaded by GitHub Action)"
            else
                log_error "Failed to create GitHub release"
                return 1
            end
        end
    end

    # Step 4.7: Verify crates.io publish (dry-run)
    log_step "Verifying crates.io publish (dry-run)"
    
    dry_run_cmd "cargo publish --dry-run"
    if test $status -ne 0
        log_error "cargo publish --dry-run failed"
        return 1
    end
    log_success "crates.io publish verification passed"

    # Step 4.8: Publish to crates.io
    log_step "Publishing to crates.io"

    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would run 'cargo publish' to publish to crates.io"
    else
        cd "$PACSEA_DIR"
        cargo publish

        if test $status -eq 0
            log_success "Published to crates.io"
        else
            log_error "Failed to publish to crates.io"
            if not confirm_continue "Continue anyway?"
                return 1
            end
        end
    end

    return 0
end

# ============================================================================
# Phase 5: AUR Update
# ============================================================================

function phase5_aur_update
    set -l new_ver $argv[1]
    
    log_phase "5. AUR Update"
    
    # Step 5.1: Change to pacsea-bin directory
    log_step "Updating AUR SHA sums"
    
    log_warn "Wait for GitHub Action 'release' to finish uploading the binary!"
    log_info "Check: https://github.com/Firstp1ck/Pacsea/actions"
    wait_for_user "Press Enter when GitHub Action has completed..."
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would change to $AUR_BIN_DIR"
        log_info "[DRY-RUN] Would run update-sha"
    else
        cd "$AUR_BIN_DIR"
        log_info "Changed to: $AUR_BIN_DIR"
        log_info "Running update-sha (interactive)..."
        
        # Run update-sha - this is interactive
        update-sha
        
        if test $status -eq 0
            log_success "SHA sums updated"
        else
            log_warn "update-sha may have failed or was cancelled"
            if not confirm_continue "Continue anyway?"
                return 1
            end
        end
    end
    
    # Step 5.2: Push to AUR
    log_step "Pushing to AUR"
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would run aur-push"
    else
        log_info "Running aur-push..."
        
        aur-push
        
        if test $status -eq 0
            log_success "Pushed to AUR"
        else
            log_warn "aur-push may have failed"
            if not confirm_continue "Continue anyway?"
                return 1
            end
        end
    end
    
    # Step 5.3: Sync PKGBUILDs back to Pacsea repo
    log_step "Syncing PKGBUILDs to Pacsea repo"
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would copy $AUR_BIN_DIR/PKGBUILD to $PACSEA_DIR/PKGBUILD-bin"
        log_info "[DRY-RUN] Would copy $AUR_GIT_DIR/PKGBUILD to $PACSEA_DIR/PKGBUILD-git"
        log_info "[DRY-RUN] Would commit and push PKGBUILD changes to Pacsea repo"
    else
        cp "$AUR_BIN_DIR/PKGBUILD" "$PACSEA_DIR/PKGBUILD-bin"
        if test $status -eq 0
            log_success "Copied PKGBUILD-bin"
        else
            log_error "Failed to copy PKGBUILD-bin"
        end
        
        cp "$AUR_GIT_DIR/PKGBUILD" "$PACSEA_DIR/PKGBUILD-git"
        if test $status -eq 0
            log_success "Copied PKGBUILD-git"
        else
            log_error "Failed to copy PKGBUILD-git"
        end
        
        # Commit and push PKGBUILD changes
        cd "$PACSEA_DIR"
        git add PKGBUILD-bin PKGBUILD-git
        git commit -m "Update PKGBUILDs for v$new_ver"
        if test $status -eq 0
            log_success "PKGBUILD changes committed"
        else
            log_warn "PKGBUILD commit failed or nothing to commit"
        end
        
        git push origin
        if test $status -eq 0
            log_success "PKGBUILD changes pushed to origin"
        else
            log_error "Failed to push PKGBUILD changes"
            if not confirm_continue "Continue anyway?"
                return 1
            end
        end
    end
    
    # Step 5.4: Push wiki changes
    log_step "Pushing Wiki Changes"
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would commit and push wiki changes"
    else
        if test -d "$WIKI_DIR"
            cd "$WIKI_DIR"
            
            # Check if there are changes to commit
            set -l wiki_status (git status --porcelain)
            if test -n "$wiki_status"
                git add -A
                git commit -m "Update wiki for v$new_ver"
                if test $status -eq 0
                    log_success "Wiki changes committed"
                else
                    log_warn "Wiki commit failed or nothing to commit"
                end
                
                git push origin
                if test $status -eq 0
                    log_success "Wiki pushed to origin"
                else
                    log_error "Failed to push wiki"
                    if not confirm_continue "Continue anyway?"
                        return 1
                    end
                end
            else
                log_info "No wiki changes to commit"
            end
        else
            log_warn "Wiki directory not found: $WIKI_DIR"
        end
    end
    
    return 0
end

# ============================================================================
# Prerequisites Check
# ============================================================================

function check_prerequisites
    log_info "Checking prerequisites..."
    
    set -l missing ""
    
    # Check for required commands
    if not command -q cursor
        set missing $missing "cursor"
    end
    
    if not command -q gh
        set missing $missing "gh"
    end
    
    if not command -q cargo
        set missing $missing "cargo"
    end
    
    if not command -q git
        set missing $missing "git"
    end
    
    if not command -q python3
        set missing $missing "python3"
    end
    
    # Check for fish functions
    if not functions -q cargo-dev
        log_warn "Fish function 'cargo-dev' not found"
    end
    
    if not functions -q update-sha
        log_warn "Fish function 'update-sha' not found"
    end
    
    if not functions -q aur-push
        log_warn "Fish function 'aur-push' not found"
    end
    
    if test -n "$missing"
        log_error "Missing required commands: $missing"
        return 1
    end
    
    # Check directories exist
    if not test -d "$PACSEA_DIR"
        log_error "Pacsea directory not found: $PACSEA_DIR"
        return 1
    end
    
    if not test -d "$AUR_BIN_DIR"
        log_error "AUR bin directory not found: $AUR_BIN_DIR"
        return 1
    end
    
    if not test -d "$AUR_GIT_DIR"
        log_error "AUR git directory not found: $AUR_GIT_DIR"
        return 1
    end
    
    log_success "All prerequisites met"
    return 0
end

# ============================================================================
# Pre-flight Checks
# ============================================================================

function check_preflight
    log_info "Running pre-flight checks..."
    
    cd "$PACSEA_DIR"
    
    # Check if on main branch
    set -l current_branch (git branch --show-current)
    if test "$current_branch" != "main"
        log_error "Not on main branch (current: $current_branch)"
        if not confirm_continue "Continue on branch '$current_branch'?"
            return 1
        end
    else
        log_success "On main branch"
    end
    
    # Check for clean working directory
    set -l git_status (git status --porcelain)
    if test -n "$git_status"
        log_error "Working directory is not clean"
        log_info "Uncommitted changes:"
        git status --short
        echo
        if not confirm_continue "Continue with uncommitted changes?"
            return 1
        end
    else
        log_success "Working directory is clean"
    end
    
    return 0
end

# ============================================================================
# SECURITY.md Update
# ============================================================================

function update_security_md
    set -l new_ver $argv[1]
    set -l security_file "$PACSEA_DIR/SECURITY.md"
    
    log_step "Updating SECURITY.md"
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would update SECURITY.md with new version $new_ver"
        return 0
    end
    
    if not test -f "$security_file"
        log_error "SECURITY.md not found at: $security_file"
        return 1
    end
    
    # Parse version to get major.minor
    set -l ver_parts (string split '.' $new_ver)
    set -l major $ver_parts[1]
    set -l minor $ver_parts[2]
    set -l major_minor "$major.$minor"
    
    # Create temporary file
    set -l tmp_file (mktemp)
    
    # Read file and process line by line
    set -l in_table false
    set -l table_updated false
    set -l header_written false
    set -l separator_written false
    
    while read -l line
        # Detect the start of the version table
        if string match -r '^\|\s*Version\s*\|\s*Supported' "$line"
            set in_table true
            set header_written false
            set separator_written false
            echo "$line" >> "$tmp_file"
            set header_written true
            continue
        end
        
        # Handle table separator line
        if test "$in_table" = true; and string match -r '^\|\s*-' "$line"
            if not test "$separator_written" = true
                echo "$line" >> "$tmp_file"
                set separator_written true
                # Add new version row after separator
                echo "| $major_minor.x   | :white_check_mark: |" >> "$tmp_file"
                set table_updated true
            end
            continue
        end
        
        # Handle table rows
        if test "$in_table" = true
            if string match -r '^\|\s*<' "$line"
                # This is the "< X.Y.Z" line - update it
                echo "| < $major_minor.0   | :x:                |" >> "$tmp_file"
                continue
            else if string match -r '^\|\s*[0-9]' "$line"
                # Another version row - skip old version entries
                continue
            else if test -z (string trim "$line")
                # Empty line - end of table
                set in_table false
                echo "$line" >> "$tmp_file"
                continue
            else if string match -r '^##' "$line"
                # New section - end of table
                set in_table false
                echo "" >> "$tmp_file"
                echo "$line" >> "$tmp_file"
                continue
            else if not string match -r '^\|' "$line"
                # Not a table line anymore
                set in_table false
                echo "$line" >> "$tmp_file"
                continue
            end
        end
        
        # Write line as-is if not in table or not a table row
        echo "$line" >> "$tmp_file"
    end < "$security_file"
    
    # Replace original file
    mv "$tmp_file" "$security_file"
    
    if test "$table_updated" = true
        log_success "SECURITY.md updated: $major_minor.x is now supported"
    else
        log_warn "Could not find version table in SECURITY.md"
        return 1
    end
    
    return 0
end

# ============================================================================
# CHANGELOG Update
# ============================================================================

function update_changelog
    set -l new_ver $argv[1]
    set -l changelog_file "$PACSEA_DIR/CHANGELOG.md"
    set -l release_file "$PACSEA_DIR/Documents/RELEASE_v$new_ver.md"
    
    log_step "Updating CHANGELOG.md"
    
    if test "$DRY_RUN" = true
        log_info "[DRY-RUN] Would update CHANGELOG.md with release notes"
        return 0
    end
    
    # Create CHANGELOG.md if it doesn't exist
    if not test -f "$changelog_file"
        log_info "Creating CHANGELOG.md..."
        echo "# Changelog

All notable changes to Pacsea will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---
" > "$changelog_file"
    end
    
    # Check if release file exists
    if not test -f "$release_file"
        log_warn "Release file not found: $release_file"
        log_warn "Skipping CHANGELOG update"
        return 0
    end
    
    # Get current date
    set -l release_date (date +%Y-%m-%d)
    
    # Create a temporary file for the new changelog
    set -l tmp_file (mktemp)
    
    # Find the first --- separator line number
    set -l header_end (grep -n '^---$' "$changelog_file" | head -1 | cut -d: -f1)
    
    if test -n "$header_end"
        # Write header (up to and including first ---)
        head -n $header_end "$changelog_file" > "$tmp_file"
        
        # Add blank line and new entry header
        echo "" >> "$tmp_file"
        echo "## [$new_ver] - $release_date" >> "$tmp_file"
        echo "" >> "$tmp_file"
        
        # Append release content (preserving newlines)
        cat "$release_file" >> "$tmp_file"
        
        # Add separator
        echo "" >> "$tmp_file"
        echo "---" >> "$tmp_file"
        
        # Append rest of changelog (after first ---)
        tail -n +$header_end "$changelog_file" | tail -n +2 >> "$tmp_file"
        
        # Replace original file
        mv "$tmp_file" "$changelog_file"
    else
        # No header found, prepend new entry
        echo "## [$new_ver] - $release_date" > "$tmp_file"
        echo "" >> "$tmp_file"
        cat "$release_file" >> "$tmp_file"
        echo "" >> "$tmp_file"
        echo "---" >> "$tmp_file"
        echo "" >> "$tmp_file"
        cat "$changelog_file" >> "$tmp_file"
        mv "$tmp_file" "$changelog_file"
    end
    
    log_success "CHANGELOG.md updated"
    return 0
end

# ============================================================================
# Main Function
# ============================================================================

function main
    set -l new_version ""
    
    # Parse arguments
    for arg in $argv
        switch $arg
            case '--dry-run'
                set DRY_RUN true
                log_warn "DRY RUN MODE - No changes will be made"
            case '-h' '--help'
                echo "Usage: release.fish [--dry-run] [version]"
                echo
                echo "Options:"
                echo "  --dry-run    Preview all changes without executing them"
                echo "  -h, --help   Show this help message"
                echo
                echo "If version is not provided, you will be prompted to enter it."
                return 0
            case '*'
                if test -z "$new_version"
                    set new_version $arg
                end
        end
    end
    
    # Print banner
    echo
    _bold_cyan; echo "╔════════════════════════════════════════════════════════════════════════╗"; _reset
    _bold_cyan; echo "║                    PACSEA RELEASE AUTOMATION                          ║"; _reset
    _bold_cyan; echo "╚════════════════════════════════════════════════════════════════════════╝"; _reset
    echo
    
    # Check prerequisites
    check_prerequisites
    if test $status -ne 0
        return 1
    end
    
    # Run pre-flight checks
    check_preflight
    if test $status -ne 0
        return 1
    end
    
    # Get version if not provided
    if test -z "$new_version"
        set -l current (get_current_version)
        _cyan; echo -n "Enter new version (current: $current): "; _reset
        read new_version
    end
    
    # Validate version
    if not validate_semver "$new_version"
        log_error "Invalid version format: $new_version (expected: X.Y.Z)"
        return 1
    end
    
    # Confirm before starting
    echo
    _blue; echo -n "[INFO] "; _reset; echo -n "Release version: "; _bold; echo $new_version; _reset
    _blue; echo -n "[INFO] "; _reset; echo -n "Current version: "; _bold; echo (get_current_version); _reset
    echo
    
    if not confirm_continue "Start release process?"
        log_info "Release cancelled"
        return 0
    end
    
    # Store old version before updating
    set -l old_version (get_current_version)
    
    # Execute phases
    phase1_version_update "$new_version"
    or return 1
    
    phase2_documentation "$new_version" "$old_version"
    or return 1
    
    phase3_pkgbuild_updates "$new_version"
    or return 1
    
    phase4_build_release "$new_version"
    or return 1
    
    phase5_aur_update "$new_version"
    or return 1
    
    # Final summary
    echo
    _bold_green; echo "╔════════════════════════════════════════════════════════════════════════╗"; _reset
    _bold_green; echo "║                    RELEASE COMPLETE! 🎉                               ║"; _reset
    _bold_green; echo "╚════════════════════════════════════════════════════════════════════════╝"; _reset
    echo
    log_success "Version $new_version has been released!"
    echo
    log_info "Don't forget to verify:"
    echo "  • GitHub release: https://github.com/Firstp1ck/Pacsea/releases"
    echo "  • GitHub Action uploaded the binary"
    echo "  • AUR packages are updated"
    echo
    
    return 0
end

# Run main
main $argv

