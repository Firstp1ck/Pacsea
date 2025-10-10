use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Color;

/// Application theme palette used by rendering code.
///
/// All colors are provided as [`ratatui::style::Color`] and are suitable for
/// direct use with widgets and styles.
#[derive(Clone, Copy)]
pub struct Theme {
    /// Primary background color for the canvas.
    pub base: Color,
    /// Slightly lighter background layer used behind panels.
    pub mantle: Color,
    /// Darkest background shade for deep contrast areas.
    pub crust: Color,
    /// Subtle surface color for component backgrounds (level 1).
    pub surface1: Color,
    /// Subtle surface color for component backgrounds (level 2).
    pub surface2: Color,
    /// Muted overlay line/border color (primary).
    pub overlay1: Color,
    /// Muted overlay line/border color (secondary).
    pub overlay2: Color,
    /// Primary foreground text color.
    pub text: Color,
    /// Secondary text for less prominent content.
    pub subtext0: Color,
    /// Tertiary text for captions and low-emphasis content.
    pub subtext1: Color,
    /// Accent color commonly used for selection and interactive highlights.
    pub sapphire: Color,
    /// Accent color for emphasized headings or selections.
    pub mauve: Color,
    /// Success/positive state color.
    pub green: Color,
    /// Warning/attention state color.
    pub yellow: Color,
    /// Error/danger state color.
    pub red: Color,
    /// Accent color for subtle emphasis and borders.
    pub lavender: Color,
}

/// User-configurable application settings parsed from `pacsea.conf`.
#[derive(Clone, Debug)]
pub struct Settings {
    /// Percentage width allocated to the Recent pane (left column).
    pub layout_left_pct: u16,
    /// Percentage width allocated to the Search pane (center column).
    pub layout_center_pct: u16,
    /// Percentage width allocated to the Install pane (right column).
    pub layout_right_pct: u16,
    /// Default value for the application's dry-run mode on startup.
    /// This can be toggled via the `--dry-run` CLI flag.
    pub app_dry_run_default: bool,
    /// Configurable key bindings parsed from `pacsea.conf`
    pub keymap: KeyMap,
    /// Initial sort mode for results list.
    pub sort_mode: crate::state::SortMode,
    /// Text appended when copying PKGBUILD to clipboard.
    pub clipboard_suffix: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            layout_left_pct: 20,
            layout_center_pct: 60,
            layout_right_pct: 20,
            app_dry_run_default: false,
            keymap: KeyMap::default(),
            sort_mode: crate::state::SortMode::RepoThenName,
            clipboard_suffix: "Check PKGBUILD and source for suspicious and malicious activities"
                .to_string(),
        }
    }
}

/// A single keyboard chord (modifiers + key).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

impl KeyChord {
    /// Return a short display label such as "Ctrl+R", "F1", "Shift+Del", "+/ ?".
    pub fn label(&self) -> String {
        let mut parts: Vec<&'static str> = Vec::new();
        if self.mods.contains(KeyModifiers::CONTROL) {
            parts.push("Ctrl");
        }
        if self.mods.contains(KeyModifiers::ALT) {
            parts.push("Alt");
        }
        if self.mods.contains(KeyModifiers::SHIFT) {
            parts.push("Shift");
        }
        if self.mods.contains(KeyModifiers::SUPER) {
            parts.push("Super");
        }
        let key = match self.code {
            KeyCode::Char(ch) => {
                // Show uppercase character for display
                let up = ch.to_ascii_uppercase();
                if up == ' ' {
                    "Space".to_string()
                } else {
                    up.to_string()
                }
            }
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::BackTab => "Shift+Tab".to_string(),
            KeyCode::Delete => "Del".to_string(),
            KeyCode::Insert => "Ins".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::PageUp => "PgUp".to_string(),
            KeyCode::PageDown => "PgDn".to_string(),
            KeyCode::Up => "↑".to_string(),
            KeyCode::Down => "↓".to_string(),
            KeyCode::Left => "←".to_string(),
            KeyCode::Right => "→".to_string(),
            KeyCode::F(n) => format!("F{n}"),
            _ => "?".to_string(),
        };
        if parts.is_empty() || matches!(self.code, KeyCode::BackTab) {
            key
        } else {
            format!("{}+{}", parts.join("+"), key)
        }
    }
}

/// Application key bindings.
/// Each action can have multiple chords.
#[derive(Clone, Debug)]
pub struct KeyMap {
    // Global
    pub help_overlay: Vec<KeyChord>,
    pub reload_theme: Vec<KeyChord>,
    pub exit: Vec<KeyChord>,
    /// Global: Show/Hide PKGBUILD viewer
    pub show_pkgbuild: Vec<KeyChord>,
    pub pane_next: Vec<KeyChord>,
    pub pane_prev: Vec<KeyChord>,
    pub pane_left: Vec<KeyChord>,
    pub pane_right: Vec<KeyChord>,

    // Search
    pub search_move_up: Vec<KeyChord>,
    pub search_move_down: Vec<KeyChord>,
    pub search_page_up: Vec<KeyChord>,
    pub search_page_down: Vec<KeyChord>,
    pub search_add: Vec<KeyChord>,
    pub search_install: Vec<KeyChord>,
    pub search_focus_left: Vec<KeyChord>,
    pub search_focus_right: Vec<KeyChord>,
    pub search_backspace: Vec<KeyChord>,

    // Search normal mode
    /// Toggle Search normal mode on/off (works from both insert/normal)
    pub search_normal_toggle: Vec<KeyChord>,
    /// Enter insert mode while in Search normal mode
    pub search_normal_insert: Vec<KeyChord>,
    /// Normal mode: extend selection to the left (default: h)
    pub search_normal_select_left: Vec<KeyChord>,
    /// Normal mode: extend selection to the right (default: l)
    pub search_normal_select_right: Vec<KeyChord>,
    /// Normal mode: delete selected text (default: d)
    pub search_normal_delete: Vec<KeyChord>,

    // Recent
    pub recent_move_up: Vec<KeyChord>,
    pub recent_move_down: Vec<KeyChord>,
    pub recent_find: Vec<KeyChord>,
    pub recent_use: Vec<KeyChord>,
    pub recent_add: Vec<KeyChord>,
    pub recent_to_search: Vec<KeyChord>,
    pub recent_focus_right: Vec<KeyChord>,
    /// Remove one entry from Recent
    pub recent_remove: Vec<KeyChord>,

    // Install
    pub install_move_up: Vec<KeyChord>,
    pub install_move_down: Vec<KeyChord>,
    pub install_confirm: Vec<KeyChord>,
    pub install_remove: Vec<KeyChord>,
    pub install_clear: Vec<KeyChord>,
    pub install_find: Vec<KeyChord>,
    pub install_to_search: Vec<KeyChord>,
    pub install_focus_left: Vec<KeyChord>,
}

impl Default for KeyMap {
    fn default() -> Self {
        use KeyCode::*;
        let none = KeyModifiers::empty();
        let ctrl = KeyModifiers::CONTROL;
        let shift = KeyModifiers::SHIFT; // retained for other bindings; not used for pane switching
        KeyMap {
            help_overlay: vec![
                KeyChord {
                    code: F(1),
                    mods: none,
                },
                KeyChord {
                    code: Char('?'),
                    mods: none,
                },
            ],
            reload_theme: vec![KeyChord {
                code: Char('r'),
                mods: ctrl,
            }],
            exit: vec![KeyChord {
                code: Char('c'),
                mods: ctrl,
            }],
            show_pkgbuild: vec![KeyChord {
                code: Char('x'),
                mods: ctrl,
            }],
            pane_next: vec![KeyChord {
                code: Tab,
                mods: none,
            }],
            pane_prev: vec![],
            pane_left: vec![KeyChord {
                code: Left,
                mods: none,
            }],
            pane_right: vec![KeyChord {
                code: Right,
                mods: none,
            }],

            search_move_up: vec![KeyChord {
                code: Up,
                mods: none,
            }],
            search_move_down: vec![KeyChord {
                code: Down,
                mods: none,
            }],
            search_page_up: vec![KeyChord {
                code: PageUp,
                mods: none,
            }],
            search_page_down: vec![KeyChord {
                code: PageDown,
                mods: none,
            }],
            search_add: vec![KeyChord {
                code: Char(' '),
                mods: none,
            }],
            search_install: vec![KeyChord {
                code: Enter,
                mods: none,
            }],
            search_focus_left: vec![KeyChord {
                code: Left,
                mods: none,
            }],
            search_focus_right: vec![KeyChord {
                code: Right,
                mods: none,
            }],
            search_backspace: vec![KeyChord {
                code: Backspace,
                mods: none,
            }],

            // Search normal mode (Vim-like)
            search_normal_toggle: vec![KeyChord {
                code: Esc,
                mods: none,
            }],
            search_normal_insert: vec![KeyChord {
                code: Char('i'),
                mods: none,
            }],
            search_normal_select_left: vec![KeyChord {
                code: Char('h'),
                mods: none,
            }],
            search_normal_select_right: vec![KeyChord {
                code: Char('l'),
                mods: none,
            }],
            search_normal_delete: vec![KeyChord {
                code: Char('d'),
                mods: none,
            }],

            recent_move_up: vec![
                KeyChord {
                    code: Char('k'),
                    mods: none,
                },
                KeyChord {
                    code: Up,
                    mods: none,
                },
            ],
            recent_move_down: vec![
                KeyChord {
                    code: Char('j'),
                    mods: none,
                },
                KeyChord {
                    code: Down,
                    mods: none,
                },
            ],
            recent_find: vec![KeyChord {
                code: Char('/'),
                mods: none,
            }],
            recent_use: vec![KeyChord {
                code: Enter,
                mods: none,
            }],
            recent_add: vec![KeyChord {
                code: Char(' '),
                mods: none,
            }],
            recent_to_search: vec![KeyChord {
                code: Esc,
                mods: none,
            }],
            recent_focus_right: vec![KeyChord {
                code: Right,
                mods: none,
            }],
            recent_remove: vec![
                KeyChord {
                    code: Char('d'),
                    mods: none,
                },
                KeyChord {
                    code: Delete,
                    mods: none,
                },
            ],

            install_move_up: vec![
                KeyChord {
                    code: Char('k'),
                    mods: none,
                },
                KeyChord {
                    code: Up,
                    mods: none,
                },
            ],
            install_move_down: vec![
                KeyChord {
                    code: Char('j'),
                    mods: none,
                },
                KeyChord {
                    code: Down,
                    mods: none,
                },
            ],
            install_confirm: vec![KeyChord {
                code: Enter,
                mods: none,
            }],
            install_remove: vec![
                KeyChord {
                    code: Delete,
                    mods: none,
                },
                KeyChord {
                    code: Char('d'),
                    mods: none,
                },
            ],
            install_clear: vec![KeyChord {
                code: Delete,
                mods: shift,
            }],
            install_find: vec![KeyChord {
                code: Char('/'),
                mods: none,
            }],
            install_to_search: vec![KeyChord {
                code: Esc,
                mods: none,
            }],
            install_focus_left: vec![KeyChord {
                code: Left,
                mods: none,
            }],
        }
    }
}
