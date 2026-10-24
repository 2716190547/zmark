use gpui::*;

#[derive(Debug, Clone)]
pub struct ThemeColors {
    pub bg: Hsla,
    pub surface: Hsla,
    pub border: Hsla,
    pub text_primary: Hsla,
    pub text_muted: Hsla,
    pub accent: Hsla,
    pub code_bg: Hsla,
    pub conflict_bg: Hsla,
    pub conflict_border: Hsla,
}

impl ThemeColors {
    pub fn dark() -> Self {
        Self {
            bg: rgb(0x121214).into(),
            surface: rgb(0x1a1a1e).into(),
            border: rgb(0x27272a).into(),
            text_primary: rgb(0xf4f4f5).into(),
            text_muted: rgb(0xa1a1aa).into(),
            accent: rgb(0x38bdf8).into(),
            code_bg: rgb(0x18181b).into(),
            conflict_bg: rgb(0x451a03).into(),
            conflict_border: rgb(0xb45309).into(),
        }
    }

    pub fn light() -> Self {
        Self {
            bg: rgb(0xfcfcfc).into(),
            surface: rgb(0xf4f4f5).into(),
            border: rgb(0xe4e4e7).into(),
            text_primary: rgb(0x18181b).into(),
            text_muted: rgb(0x71717a).into(),
            accent: rgb(0x0284c7).into(),
            code_bg: rgb(0xf1f5f9).into(),
            conflict_bg: rgb(0xfef3c7).into(),
            conflict_border: rgb(0xf59e0b).into(),
        }
    }
}
