//! Core-owned neutral keyboard and mouse vocabulary.
//!
//! These tables are the single source for the `KEY` and `MOUSE` input bodies,
//! the generated TypeScript name tables, and the explicit Ghostty maps in
//! `botster-terminal-ghostty`. Values are frozen `u16` or `u8` wire numbers.
//! No Crossterm and no browser types appear here.

/// Define a `u16` key enum with explicit wire values and W3C `code` names.
macro_rules! key_enum {
    (
        $(#[$enum_meta:meta])*
        pub enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident = $value:literal => $code:literal
            ),+ $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u16)]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant = $value,
            )+
        }

        impl $name {
            /// Every published key, in wire-value order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Decode a wire value.
            #[must_use]
            pub const fn from_u16(value: u16) -> Option<Self> {
                match value {
                    $($value => Some(Self::$variant),)+
                    _ => None,
                }
            }

            /// Wire value.
            #[must_use]
            pub const fn as_u16(self) -> u16 {
                self as u16
            }

            /// W3C UI Events `KeyboardEvent.code` name for this key.
            #[must_use]
            pub const fn code(self) -> &'static str {
                match self {
                    $(Self::$variant => $code,)+
                }
            }

            /// Look up a key by its W3C `code` name.
            #[must_use]
            pub fn from_code(code: &str) -> Option<Self> {
                match code {
                    $($code => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

key_enum! {
    /// Physical key identity, layout independent.
    ///
    /// Values follow the W3C UI Events `code` inventory that Ghostty
    /// understands. `Unidentified` is `0`. The Ghostty adapter maps every
    /// variant through an explicit `match`, never by numeric order.
    pub enum TerminalKey {
        /// No physical key identity. The encoder uses `utf8` only.
        Unidentified = 0 => "Unidentified",
        /// `` ` `` key.
        Backquote = 1 => "Backquote",
        /// `\` key.
        Backslash = 2 => "Backslash",
        /// `[` key.
        BracketLeft = 3 => "BracketLeft",
        /// `]` key.
        BracketRight = 4 => "BracketRight",
        /// `,` key.
        Comma = 5 => "Comma",
        /// `0` on the main row.
        Digit0 = 6 => "Digit0",
        /// `1` on the main row.
        Digit1 = 7 => "Digit1",
        /// `2` on the main row.
        Digit2 = 8 => "Digit2",
        /// `3` on the main row.
        Digit3 = 9 => "Digit3",
        /// `4` on the main row.
        Digit4 = 10 => "Digit4",
        /// `5` on the main row.
        Digit5 = 11 => "Digit5",
        /// `6` on the main row.
        Digit6 = 12 => "Digit6",
        /// `7` on the main row.
        Digit7 = 13 => "Digit7",
        /// `8` on the main row.
        Digit8 = 14 => "Digit8",
        /// `9` on the main row.
        Digit9 = 15 => "Digit9",
        /// `=` key.
        Equal = 16 => "Equal",
        /// Extra key left of `Z` on ISO layouts.
        IntlBackslash = 17 => "IntlBackslash",
        /// Japanese `ろ` key.
        IntlRo = 18 => "IntlRo",
        /// Japanese `¥` key.
        IntlYen = 19 => "IntlYen",
        /// `A` key.
        KeyA = 20 => "KeyA",
        /// `B` key.
        KeyB = 21 => "KeyB",
        /// `C` key.
        KeyC = 22 => "KeyC",
        /// `D` key.
        KeyD = 23 => "KeyD",
        /// `E` key.
        KeyE = 24 => "KeyE",
        /// `F` key.
        KeyF = 25 => "KeyF",
        /// `G` key.
        KeyG = 26 => "KeyG",
        /// `H` key.
        KeyH = 27 => "KeyH",
        /// `I` key.
        KeyI = 28 => "KeyI",
        /// `J` key.
        KeyJ = 29 => "KeyJ",
        /// `K` key.
        KeyK = 30 => "KeyK",
        /// `L` key.
        KeyL = 31 => "KeyL",
        /// `M` key.
        KeyM = 32 => "KeyM",
        /// `N` key.
        KeyN = 33 => "KeyN",
        /// `O` key.
        KeyO = 34 => "KeyO",
        /// `P` key.
        KeyP = 35 => "KeyP",
        /// `Q` key.
        KeyQ = 36 => "KeyQ",
        /// `R` key.
        KeyR = 37 => "KeyR",
        /// `S` key.
        KeyS = 38 => "KeyS",
        /// `T` key.
        KeyT = 39 => "KeyT",
        /// `U` key.
        KeyU = 40 => "KeyU",
        /// `V` key.
        KeyV = 41 => "KeyV",
        /// `W` key.
        KeyW = 42 => "KeyW",
        /// `X` key.
        KeyX = 43 => "KeyX",
        /// `Y` key.
        KeyY = 44 => "KeyY",
        /// `Z` key.
        KeyZ = 45 => "KeyZ",
        /// `-` key.
        Minus = 46 => "Minus",
        /// `.` key.
        Period = 47 => "Period",
        /// `'` key.
        Quote = 48 => "Quote",
        /// `;` key.
        Semicolon = 49 => "Semicolon",
        /// `/` key.
        Slash = 50 => "Slash",
        /// Left Alt or Option.
        AltLeft = 51 => "AltLeft",
        /// Right Alt or Option.
        AltRight = 52 => "AltRight",
        /// Backspace.
        Backspace = 53 => "Backspace",
        /// Caps Lock.
        CapsLock = 54 => "CapsLock",
        /// Context menu.
        ContextMenu = 55 => "ContextMenu",
        /// Left Control.
        ControlLeft = 56 => "ControlLeft",
        /// Right Control.
        ControlRight = 57 => "ControlRight",
        /// Enter or Return.
        Enter = 58 => "Enter",
        /// Left Meta, Command, or Windows.
        MetaLeft = 59 => "MetaLeft",
        /// Right Meta, Command, or Windows.
        MetaRight = 60 => "MetaRight",
        /// Left Shift.
        ShiftLeft = 61 => "ShiftLeft",
        /// Right Shift.
        ShiftRight = 62 => "ShiftRight",
        /// Space bar.
        Space = 63 => "Space",
        /// Tab.
        Tab = 64 => "Tab",
        /// Japanese conversion.
        Convert = 65 => "Convert",
        /// Japanese kana mode.
        KanaMode = 66 => "KanaMode",
        /// Japanese non-conversion.
        NonConvert = 67 => "NonConvert",
        /// Forward delete.
        Delete = 68 => "Delete",
        /// End.
        End = 69 => "End",
        /// Help.
        Help = 70 => "Help",
        /// Home.
        Home = 71 => "Home",
        /// Insert.
        Insert = 72 => "Insert",
        /// Page Down.
        PageDown = 73 => "PageDown",
        /// Page Up.
        PageUp = 74 => "PageUp",
        /// Down arrow.
        ArrowDown = 75 => "ArrowDown",
        /// Left arrow.
        ArrowLeft = 76 => "ArrowLeft",
        /// Right arrow.
        ArrowRight = 77 => "ArrowRight",
        /// Up arrow.
        ArrowUp = 78 => "ArrowUp",
        /// Num Lock.
        NumLock = 79 => "NumLock",
        /// Numpad `0`.
        Numpad0 = 80 => "Numpad0",
        /// Numpad `1`.
        Numpad1 = 81 => "Numpad1",
        /// Numpad `2`.
        Numpad2 = 82 => "Numpad2",
        /// Numpad `3`.
        Numpad3 = 83 => "Numpad3",
        /// Numpad `4`.
        Numpad4 = 84 => "Numpad4",
        /// Numpad `5`.
        Numpad5 = 85 => "Numpad5",
        /// Numpad `6`.
        Numpad6 = 86 => "Numpad6",
        /// Numpad `7`.
        Numpad7 = 87 => "Numpad7",
        /// Numpad `8`.
        Numpad8 = 88 => "Numpad8",
        /// Numpad `9`.
        Numpad9 = 89 => "Numpad9",
        /// Numpad `+`.
        NumpadAdd = 90 => "NumpadAdd",
        /// Numpad backspace.
        NumpadBackspace = 91 => "NumpadBackspace",
        /// Numpad clear.
        NumpadClear = 92 => "NumpadClear",
        /// Numpad clear entry.
        NumpadClearEntry = 93 => "NumpadClearEntry",
        /// Numpad `,`.
        NumpadComma = 94 => "NumpadComma",
        /// Numpad `.`.
        NumpadDecimal = 95 => "NumpadDecimal",
        /// Numpad `/`.
        NumpadDivide = 96 => "NumpadDivide",
        /// Numpad Enter.
        NumpadEnter = 97 => "NumpadEnter",
        /// Numpad `=`.
        NumpadEqual = 98 => "NumpadEqual",
        /// Numpad memory add.
        NumpadMemoryAdd = 99 => "NumpadMemoryAdd",
        /// Numpad memory clear.
        NumpadMemoryClear = 100 => "NumpadMemoryClear",
        /// Numpad memory recall.
        NumpadMemoryRecall = 101 => "NumpadMemoryRecall",
        /// Numpad memory store.
        NumpadMemoryStore = 102 => "NumpadMemoryStore",
        /// Numpad memory subtract.
        NumpadMemorySubtract = 103 => "NumpadMemorySubtract",
        /// Numpad `*`.
        NumpadMultiply = 104 => "NumpadMultiply",
        /// Numpad `(`.
        NumpadParenLeft = 105 => "NumpadParenLeft",
        /// Numpad `)`.
        NumpadParenRight = 106 => "NumpadParenRight",
        /// Numpad `-`.
        NumpadSubtract = 107 => "NumpadSubtract",
        /// Numpad separator.
        NumpadSeparator = 108 => "NumpadSeparator",
        /// Numpad up with Num Lock off.
        NumpadUp = 109 => "NumpadUp",
        /// Numpad down with Num Lock off.
        NumpadDown = 110 => "NumpadDown",
        /// Numpad right with Num Lock off.
        NumpadRight = 111 => "NumpadRight",
        /// Numpad left with Num Lock off.
        NumpadLeft = 112 => "NumpadLeft",
        /// Numpad begin (`5`) with Num Lock off.
        NumpadBegin = 113 => "NumpadBegin",
        /// Numpad home with Num Lock off.
        NumpadHome = 114 => "NumpadHome",
        /// Numpad end with Num Lock off.
        NumpadEnd = 115 => "NumpadEnd",
        /// Numpad insert with Num Lock off.
        NumpadInsert = 116 => "NumpadInsert",
        /// Numpad delete with Num Lock off.
        NumpadDelete = 117 => "NumpadDelete",
        /// Numpad page up with Num Lock off.
        NumpadPageUp = 118 => "NumpadPageUp",
        /// Numpad page down with Num Lock off.
        NumpadPageDown = 119 => "NumpadPageDown",
        /// Escape.
        Escape = 120 => "Escape",
        /// F1.
        F1 = 121 => "F1",
        /// F2.
        F2 = 122 => "F2",
        /// F3.
        F3 = 123 => "F3",
        /// F4.
        F4 = 124 => "F4",
        /// F5.
        F5 = 125 => "F5",
        /// F6.
        F6 = 126 => "F6",
        /// F7.
        F7 = 127 => "F7",
        /// F8.
        F8 = 128 => "F8",
        /// F9.
        F9 = 129 => "F9",
        /// F10.
        F10 = 130 => "F10",
        /// F11.
        F11 = 131 => "F11",
        /// F12.
        F12 = 132 => "F12",
        /// F13.
        F13 = 133 => "F13",
        /// F14.
        F14 = 134 => "F14",
        /// F15.
        F15 = 135 => "F15",
        /// F16.
        F16 = 136 => "F16",
        /// F17.
        F17 = 137 => "F17",
        /// F18.
        F18 = 138 => "F18",
        /// F19.
        F19 = 139 => "F19",
        /// F20.
        F20 = 140 => "F20",
        /// F21.
        F21 = 141 => "F21",
        /// F22.
        F22 = 142 => "F22",
        /// F23.
        F23 = 143 => "F23",
        /// F24.
        F24 = 144 => "F24",
        /// F25.
        F25 = 145 => "F25",
        /// Fn.
        Fn = 146 => "Fn",
        /// Fn Lock.
        FnLock = 147 => "FnLock",
        /// Print Screen.
        PrintScreen = 148 => "PrintScreen",
        /// Scroll Lock.
        ScrollLock = 149 => "ScrollLock",
        /// Pause.
        Pause = 150 => "Pause",
        /// Browser back.
        BrowserBack = 151 => "BrowserBack",
        /// Browser favorites.
        BrowserFavorites = 152 => "BrowserFavorites",
        /// Browser forward.
        BrowserForward = 153 => "BrowserForward",
        /// Browser home.
        BrowserHome = 154 => "BrowserHome",
        /// Browser refresh.
        BrowserRefresh = 155 => "BrowserRefresh",
        /// Browser search.
        BrowserSearch = 156 => "BrowserSearch",
        /// Browser stop.
        BrowserStop = 157 => "BrowserStop",
        /// Eject.
        Eject = 158 => "Eject",
        /// Launch application 1.
        LaunchApp1 = 159 => "LaunchApp1",
        /// Launch application 2.
        LaunchApp2 = 160 => "LaunchApp2",
        /// Launch mail.
        LaunchMail = 161 => "LaunchMail",
        /// Media play or pause.
        MediaPlayPause = 162 => "MediaPlayPause",
        /// Media select.
        MediaSelect = 163 => "MediaSelect",
        /// Media stop.
        MediaStop = 164 => "MediaStop",
        /// Media next track.
        MediaTrackNext = 165 => "MediaTrackNext",
        /// Media previous track.
        MediaTrackPrevious = 166 => "MediaTrackPrevious",
        /// Power.
        Power = 167 => "Power",
        /// Sleep.
        Sleep = 168 => "Sleep",
        /// Volume down.
        AudioVolumeDown = 169 => "AudioVolumeDown",
        /// Volume mute.
        AudioVolumeMute = 170 => "AudioVolumeMute",
        /// Volume up.
        AudioVolumeUp = 171 => "AudioVolumeUp",
        /// Wake up.
        WakeUp = 172 => "WakeUp",
        /// Copy.
        Copy = 173 => "Copy",
        /// Cut.
        Cut = 174 => "Cut",
        /// Paste.
        Paste = 175 => "Paste",
    }
}

/// Define a `u8` action or button enum with explicit wire values.
macro_rules! small_enum {
    (
        $(#[$enum_meta:meta])*
        pub enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident = $value:literal => $wire:literal
            ),+ $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(u8)]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant = $value,
            )+
        }

        impl $name {
            /// Every published variant, in wire-value order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Decode a wire byte.
            #[must_use]
            pub const fn from_byte(byte: u8) -> Option<Self> {
                match byte {
                    $($value => Some(Self::$variant),)+
                    _ => None,
                }
            }

            /// Wire byte.
            #[must_use]
            pub const fn as_byte(self) -> u8 {
                self as u8
            }

            /// Stable snake_case name used by generated TypeScript.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire,)+
                }
            }
        }
    };
}

small_enum! {
    /// `KEY` action byte.
    pub enum TerminalKeyAction {
        /// Key pressed.
        Press = 1 => "press",
        /// Key released.
        Release = 2 => "release",
        /// Key held and repeating.
        Repeat = 3 => "repeat",
    }
}

small_enum! {
    /// `MOUSE` action byte.
    pub enum TerminalMouseAction {
        /// Button pressed. Wheel steps are presses of buttons 4 through 7.
        Press = 1 => "press",
        /// Button released.
        Release = 2 => "release",
        /// Pointer moved. `has_button` reports a held button, if any.
        Motion = 3 => "motion",
    }
}

small_enum! {
    /// `MOUSE` button byte. Present only when `has_button` is `1`.
    pub enum TerminalMouseButton {
        /// Primary button.
        Left = 1 => "left",
        /// Secondary button.
        Right = 2 => "right",
        /// Middle button.
        Middle = 3 => "middle",
        /// Wheel up.
        WheelUp = 4 => "wheel_up",
        /// Wheel down.
        WheelDown = 5 => "wheel_down",
        /// Wheel left.
        WheelLeft = 6 => "wheel_left",
        /// Wheel right.
        WheelRight = 7 => "wheel_right",
        /// Extra button 8.
        Button8 = 8 => "button8",
        /// Extra button 9.
        Button9 = 9 => "button9",
        /// Extra button 10.
        Button10 = 10 => "button10",
        /// Extra button 11.
        Button11 = 11 => "button11",
    }
}

/// Modifier bit flags for `KEY` and `MOUSE` bodies (`u16`).
///
/// Side bits are meaningful only when the matching modifier bit is set.
pub mod terminal_mods {
    /// Shift is held.
    pub const SHIFT: u16 = 1 << 0;
    /// Control is held.
    pub const CTRL: u16 = 1 << 1;
    /// Alt or Option is held.
    pub const ALT: u16 = 1 << 2;
    /// Super, Command, or Windows is held.
    pub const SUPER: u16 = 1 << 3;
    /// Caps Lock is active.
    pub const CAPS_LOCK: u16 = 1 << 4;
    /// Num Lock is active.
    pub const NUM_LOCK: u16 = 1 << 5;
    /// Right Shift (1) instead of left (0).
    pub const SHIFT_SIDE: u16 = 1 << 6;
    /// Right Control (1) instead of left (0).
    pub const CTRL_SIDE: u16 = 1 << 7;
    /// Right Alt (1) instead of left (0).
    pub const ALT_SIDE: u16 = 1 << 8;
    /// Right Super (1) instead of left (0).
    pub const SUPER_SIDE: u16 = 1 << 9;

    /// Named bits in definition order, for generated tables.
    pub const ALL: &[(&str, u16)] = &[
        ("SHIFT", SHIFT),
        ("CTRL", CTRL),
        ("ALT", ALT),
        ("SUPER", SUPER),
        ("CAPS_LOCK", CAPS_LOCK),
        ("NUM_LOCK", NUM_LOCK),
        ("SHIFT_SIDE", SHIFT_SIDE),
        ("CTRL_SIDE", CTRL_SIDE),
        ("ALT_SIDE", ALT_SIDE),
        ("SUPER_SIDE", SUPER_SIDE),
    ];
}
