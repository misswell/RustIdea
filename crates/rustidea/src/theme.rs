use gpui::{Hsla, rgb};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

impl ThemeMode {
    pub fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub mode: ThemeMode,
    pub window: Hsla,
    pub titlebar: Hsla,
    pub panel: Hsla,
    pub panel_alt: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub faint: Hsla,
    pub accent: Hsla,
    pub accent_muted: Hsla,
    pub selection: Hsla,
    pub current_line: Hsla,
    pub gutter: Hsla,
    pub cursor: Hsla,
    pub tab_active: Hsla,
    pub warning: Hsla,
    pub error: Hsla,
}

impl Theme {
    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self {
                mode,
                window: rgb(0x17181b).into(),
                titlebar: rgb(0x1f2024).into(),
                panel: rgb(0x1b1c20).into(),
                panel_alt: rgb(0x222329).into(),
                border: rgb(0x30323a).into(),
                text: rgb(0xe6e7eb).into(),
                muted: rgb(0x9b9ea9).into(),
                faint: rgb(0x6f737e).into(),
                accent: rgb(0x78a9ff).into(),
                accent_muted: rgb(0x243b5f).into(),
                selection: rgb(0x2e4c78).into(),
                current_line: rgb(0x202127).into(),
                gutter: rgb(0x17181b).into(),
                cursor: rgb(0xa9c7ff).into(),
                tab_active: rgb(0x17181b).into(),
                warning: rgb(0xe8b667).into(),
                error: rgb(0xf07a80).into(),
            },
            ThemeMode::Light => Self {
                mode,
                window: rgb(0xf7f8fa).into(),
                titlebar: rgb(0xe9ebef).into(),
                panel: rgb(0xf0f2f5).into(),
                panel_alt: rgb(0xe7e9ed).into(),
                border: rgb(0xd2d6dd).into(),
                text: rgb(0x25282f).into(),
                muted: rgb(0x606773).into(),
                faint: rgb(0x8a919d).into(),
                accent: rgb(0x2767c7).into(),
                accent_muted: rgb(0xdce8fb).into(),
                selection: rgb(0xcfe0fc).into(),
                current_line: rgb(0xf1f3f6).into(),
                gutter: rgb(0xf7f8fa).into(),
                cursor: rgb(0x184a9a).into(),
                tab_active: rgb(0xf7f8fa).into(),
                warning: rgb(0x9b6500).into(),
                error: rgb(0xbc2f3b).into(),
            },
        }
    }
}
