use gpui::*;

#[derive(Debug, Clone)]
pub struct ThemeColors {
    #[allow(dead_code)]
    pub id: &'static str,
    #[allow(dead_code)]
    pub name: &'static str,
    pub is_dark: bool,
    pub bg: Hsla,
    pub surface: Hsla,
    pub border: Hsla,
    pub text_primary: Hsla,
    pub text_muted: Hsla,
    pub accent: Hsla,
    pub code_bg: Hsla,
    pub conflict_bg: Hsla,
    pub conflict_border: Hsla,
    #[allow(dead_code)]
    pub syntax_keyword: Hsla,
    #[allow(dead_code)]
    pub syntax_string: Hsla,
    #[allow(dead_code)]
    pub syntax_comment: Hsla,
    #[allow(dead_code)]
    pub syntax_heading: Hsla,
    pub syntax_inline_code: Hsla,
    #[allow(dead_code)]
    pub syntax_link: Hsla,
}

pub struct PaletteDescriptor {
    pub id: &'static str,
    #[allow(dead_code)]
    pub name: &'static str,
    pub preview_color: Hsla,
}

pub const ALL_PALETTES: &[PaletteDescriptor] = &[
    PaletteDescriptor { id: "default", name: "Default", preview_color: Hsla { h: 12.0 / 360.0, s: 0.70, l: 0.52, a: 1.0 } },
    PaletteDescriptor { id: "claude", name: "Claude Warm", preview_color: Hsla { h: 35.0 / 360.0, s: 0.9, l: 0.5, a: 1.0 } },
    PaletteDescriptor { id: "macos", name: "macOS Vibrant", preview_color: Hsla { h: 211.0 / 360.0, s: 1.0, l: 0.45, a: 1.0 } },
    PaletteDescriptor { id: "tokyo-night", name: "Tokyo Night", preview_color: Hsla { h: 224.0 / 360.0, s: 0.88, l: 0.72, a: 1.0 } },
    PaletteDescriptor { id: "nord", name: "Nord", preview_color: Hsla { h: 193.0 / 360.0, s: 0.43, l: 0.67, a: 1.0 } },
    PaletteDescriptor { id: "github", name: "GitHub Light", preview_color: Hsla { h: 212.0 / 360.0, s: 0.92, l: 0.45, a: 1.0 } },
    PaletteDescriptor { id: "solarized", name: "Solarized Light", preview_color: Hsla { h: 44.0 / 360.0, s: 0.85, l: 0.5, a: 1.0 } },
    PaletteDescriptor { id: "one-dark", name: "One Dark Pro", preview_color: Hsla { h: 220.0 / 360.0, s: 0.60, l: 0.55, a: 1.0 } },
];

pub trait HslaExt {
    fn lighten(&self, amount: f32) -> Hsla;
}

impl HslaExt for Hsla {
    fn lighten(&self, amount: f32) -> Hsla {
        Hsla {
            h: self.h,
            s: self.s,
            l: (self.l + amount).clamp(0.0, 1.0),
            a: self.a,
        }
    }
}

impl ThemeColors {
    /// Low-contrast cool neutral pill background: `#EFF1F4` (light) / `#272B33` (dark)
    pub fn warm_pill_bg(&self) -> Hsla {
        if self.is_dark {
            rgb(0x272b33).into()
        } else {
            rgb(0xeff1f4).into()
        }
    }

    /// Soft translucent hover highlight: low-contrast warm stone tint
    pub fn warm_pill_hover(&self) -> Hsla {
        if self.is_dark {
            rgba(0xffffff0f).into() // ~6% white in dark mode
        } else {
            rgba(0x0000000d).into() // ~5% black in light mode
        }
    }

    /// Primary text for subtle pill: `#1F2328` (light) / `#F0F2F5` (dark)
    pub fn warm_pill_text(&self) -> Hsla {
        if self.is_dark {
            rgb(0xf0f2f5).into()
        } else {
            rgb(0x1f2328).into()
        }
    }

    /// Adaptive segmented control track background:
    /// - Light mode: `text_primary.opacity(0.06)`
    /// - Dark mode: `surface.lighten(0.03)`
    pub fn segmented_track_bg(&self) -> Hsla {
        if self.is_dark {
            self.surface.lighten(0.03)
        } else {
            self.text_primary.opacity(0.06)
        }
    }

    /// Adaptive segmented control active thumb background:
    /// - Light mode: `surface` (pure white card)
    /// - Dark mode: `surface.lighten(0.08)` (bright embossed layer)
    pub fn segmented_thumb_bg(&self) -> Hsla {
        if self.is_dark {
            self.surface.lighten(0.08)
        } else {
            self.surface
        }
    }

    /// Adaptive segmented control active thumb hairline border:
    /// - Light mode: `border.opacity(0.45)`
    /// - Dark mode: `border.opacity(0.65)`
    pub fn segmented_thumb_border(&self) -> Hsla {
        if self.is_dark {
            self.border.opacity(0.65)
        } else {
            self.border.opacity(0.45)
        }
    }

    /// Adaptive segmented control active thumb text color:
    /// High-contrast primary text color.
    pub fn segmented_thumb_text(&self) -> Hsla {
        self.text_primary
    }

    /// Adaptive segmented control inactive option text color:
    /// Muted text color.
    pub fn segmented_inactive_text(&self) -> Hsla {
        self.text_muted
    }

    pub fn resolve(palette_id: &str, is_dark: bool) -> Self {
        match palette_id {
            "default" | "miaoyan" => Self::default_palette(is_dark),
            "claude" => Self::claude_warm(is_dark),
            "macos" => Self::macos(is_dark),
            "tokyo-night" => Self::tokyo_night(is_dark),
            "nord" => Self::nord(is_dark),
            "github" => Self::github(is_dark),
            "solarized" => Self::solarized(is_dark),
            "one-dark" => Self::one_dark(is_dark),
            _ => Self::default_palette(is_dark),
        }
    }

    pub fn default_palette(is_dark: bool) -> Self {
        if is_dark {
            Self {
                id: "default",
                name: "Default",
                is_dark: true,
                bg: rgb(0x181716).into(),      // Warm deep espresso (#181716)
                surface: rgb(0x21201e).into(), // Warm dark stone surface (#21201E)
                border: rgb(0x32302d).into(),  // Warm dark hairline divider (#32302D)
                text_primary: rgb(0xf3f1ed).into(), // Warm light stone text (#F3F1ED)
                text_muted: rgb(0x938e86).into(),   // Warm muted secondary text (#938E86)
                accent: rgb(0xe26543).into(),  // Warm terracotta coral highlight (#E26543)
                code_bg: rgb(0x282623).into(),
                conflict_bg: rgb(0x382218).into(),
                conflict_border: rgb(0x5e3322).into(),
                syntax_keyword: rgb(0xd2a8ff).into(),
                syntax_string: rgb(0x7ee787).into(),
                syntax_comment: rgb(0x8b949e).into(),
                syntax_heading: rgb(0x58a6ff).into(),
                syntax_inline_code: rgb(0xf3f1ed).into(),
                syntax_link: rgb(0x58a6ff).into(),
            }
        } else {
            Self {
                id: "default",
                name: "Default",
                is_dark: false,
                bg: rgb(0xf5f4f0).into(),      // Warm stone paper for window & sidebar (#F5F4F0)
                surface: rgb(0xffffff).into(), // Pure crisp white paper canvas (#FFFFFF)
                border: rgb(0xe6e4de).into(),  // Delicate warm stone hairline divider (#E6E4DE)
                text_primary: rgb(0x1f2328).into(), // Deep carbon gray text (#1F2328)
                text_muted: rgb(0x787571).into(),   // Warm neutral muted text (#787571)
                accent: rgb(0xd9532f).into(),  // Warm Terracotta Coral matching Logo (#D9532F)
                code_bg: rgb(0xf6f8fa).into(), // Clean light gray code block (#F6F8FA)
                conflict_bg: rgb(0xfef3e8).into(),
                conflict_border: rgb(0xfcd9b8).into(),
                syntax_keyword: rgb(0x8250df).into(),
                syntax_string: rgb(0x1a7f37).into(),
                syntax_comment: rgb(0x6e7781).into(),
                syntax_heading: rgb(0x0969da).into(),
                syntax_inline_code: rgb(0x1f2328).into(),
                syntax_link: rgb(0x0969da).into(),
            }
        }
    }

    pub fn claude_warm(is_dark: bool) -> Self {
        if is_dark {
            Self {
                id: "claude",
                name: "Claude Warm",
                is_dark: true,
                bg: rgb(0x1a1917).into(),
                surface: rgb(0x24221f).into(),
                border: rgb(0x36332e).into(),
                text_primary: rgb(0xeae7e1).into(),
                text_muted: rgb(0x9e978c).into(),
                accent: rgb(0xf59e0b).into(),
                code_bg: rgb(0x2c2925).into(),
                conflict_bg: rgb(0x451a03).into(),
                conflict_border: rgb(0xb45309).into(),
                syntax_keyword: rgb(0xfbbf24).into(),
                syntax_string: rgb(0x34d399).into(),
                syntax_comment: rgb(0xa8a29e).into(),
                syntax_heading: rgb(0xf59e0b).into(),
                syntax_inline_code: rgb(0xfafaf9).into(),
                syntax_link: rgb(0xfbbf24).into(),
            }
        } else {
            Self {
                id: "claude",
                name: "Claude Warm",
                is_dark: false,
                bg: rgb(0xfaf9f6).into(),
                surface: rgb(0xf3efea).into(),
                border: rgb(0xe6dfd5).into(),
                text_primary: rgb(0x2a2723).into(),
                text_muted: rgb(0x797369).into(),
                accent: rgb(0xd97706).into(),
                code_bg: rgb(0xefece6).into(),
                conflict_bg: rgb(0xfef3c7).into(),
                conflict_border: rgb(0xf59e0b).into(),
                syntax_keyword: rgb(0xd97706).into(),
                syntax_string: rgb(0x059669).into(),
                syntax_comment: rgb(0x78716c).into(),
                syntax_heading: rgb(0xb45309).into(),
                syntax_inline_code: rgb(0x292524).into(),
                syntax_link: rgb(0xb45309).into(),
            }
        }
    }

    pub fn macos(is_dark: bool) -> Self {
        if is_dark {
            Self {
                id: "macos",
                name: "macOS Vibrant",
                is_dark: true,
                bg: rgb(0x1c1c1e).into(),
                surface: rgb(0x2c2c2e).into(),
                border: rgb(0x3a3a3c).into(),
                text_primary: rgb(0xf5f5f7).into(),
                text_muted: rgb(0x8e8e93).into(),
                accent: rgb(0x2997ff).into(),
                code_bg: rgb(0x252528).into(),
                conflict_bg: rgb(0x451a03).into(),
                conflict_border: rgb(0xb45309).into(),
                syntax_keyword: rgb(0xbf5af2).into(),
                syntax_string: rgb(0x30d158).into(),
                syntax_comment: rgb(0x98989d).into(),
                syntax_heading: rgb(0x0a84ff).into(),
                syntax_inline_code: rgb(0xffffff).into(),
                syntax_link: rgb(0x0a84ff).into(),
            }
        } else {
            Self {
                id: "macos",
                name: "macOS Vibrant",
                is_dark: false,
                bg: rgb(0xeeeeef).into(),
                surface: rgb(0xffffff).into(),
                border: rgb(0xdcdce0).into(),
                text_primary: rgb(0x1d1d1f).into(),
                text_muted: rgb(0x86868b).into(),
                accent: rgb(0x0071e3).into(),
                code_bg: rgb(0xf0f0f2).into(),
                conflict_bg: rgb(0xfef3c7).into(),
                conflict_border: rgb(0xf59e0b).into(),
                syntax_keyword: rgb(0xaf52de).into(),
                syntax_string: rgb(0x34c759).into(),
                syntax_comment: rgb(0x8e8e93).into(),
                syntax_heading: rgb(0x007aff).into(),
                syntax_inline_code: rgb(0x1c1c1e).into(),
                syntax_link: rgb(0x007aff).into(),
            }
        }
    }

    pub fn tokyo_night(is_dark: bool) -> Self {
        if is_dark {
            Self {
                id: "tokyo-night",
                name: "Tokyo Night",
                is_dark: true,
                bg: rgb(0x1a1b26).into(),
                surface: rgb(0x1f2335).into(),
                border: rgb(0x292e42).into(),
                text_primary: rgb(0xc0caf5).into(),
                text_muted: rgb(0x7aa2f7).into(),
                accent: rgb(0x7aa2f7).into(),
                code_bg: rgb(0x24283b).into(),
                conflict_bg: rgb(0x441b22).into(),
                conflict_border: rgb(0xf7768e).into(),
                syntax_keyword: rgb(0xbb9af7).into(),
                syntax_string: rgb(0x9ece6a).into(),
                syntax_comment: rgb(0x565f89).into(),
                syntax_heading: rgb(0x7aa2f7).into(),
                syntax_inline_code: rgb(0xc0caf5).into(),
                syntax_link: rgb(0x7dcfff).into(),
            }
        } else {
            Self {
                id: "tokyo-night",
                name: "Tokyo Night",
                is_dark: false,
                bg: rgb(0xe1e2e7).into(),
                surface: rgb(0xd5d6db).into(),
                border: rgb(0xb4b5b9).into(),
                text_primary: rgb(0x343b58).into(),
                text_muted: rgb(0x565f89).into(),
                accent: rgb(0x3d59a1).into(),
                code_bg: rgb(0xcbccd1).into(),
                conflict_bg: rgb(0xfde8e8).into(),
                conflict_border: rgb(0xf7768e).into(),
                syntax_keyword: rgb(0x7847bd).into(),
                syntax_string: rgb(0x485e30).into(),
                syntax_comment: rgb(0x8990b3).into(),
                syntax_heading: rgb(0x2e7de9).into(),
                syntax_inline_code: rgb(0x343b58).into(),
                syntax_link: rgb(0x2e7de9).into(),
            }
        }
    }

    pub fn nord(is_dark: bool) -> Self {
        if is_dark {
            Self {
                id: "nord",
                name: "Nord",
                is_dark: true,
                bg: rgb(0x2e3440).into(),
                surface: rgb(0x3b4252).into(),
                border: rgb(0x434c5e).into(),
                text_primary: rgb(0xeceff4).into(),
                text_muted: rgb(0xd8dee9).into(),
                accent: rgb(0x88c0d0).into(),
                code_bg: rgb(0x3b4252).into(),
                conflict_bg: rgb(0x4c3238).into(),
                conflict_border: rgb(0xbf616a).into(),
                syntax_keyword: rgb(0xb48ead).into(),
                syntax_string: rgb(0xa3be8c).into(),
                syntax_comment: rgb(0x616e88).into(),
                syntax_heading: rgb(0x81a1c1).into(),
                syntax_inline_code: rgb(0xeceff4).into(),
                syntax_link: rgb(0x88c0d0).into(),
            }
        } else {
            Self {
                id: "nord",
                name: "Nord",
                is_dark: false,
                bg: rgb(0xeceff4).into(),
                surface: rgb(0xe5e9f0).into(),
                border: rgb(0xd8dee9).into(),
                text_primary: rgb(0x2e3440).into(),
                text_muted: rgb(0x4c566a).into(),
                accent: rgb(0x5e81ac).into(),
                code_bg: rgb(0xe5e9f0).into(),
                conflict_bg: rgb(0xfbedee).into(),
                conflict_border: rgb(0xbf616a).into(),
                syntax_keyword: rgb(0xb48ead).into(),
                syntax_string: rgb(0xa3be8c).into(),
                syntax_comment: rgb(0x4c566a).into(),
                syntax_heading: rgb(0x5e81ac).into(),
                syntax_inline_code: rgb(0x2e3440).into(),
                syntax_link: rgb(0x88c0d0).into(),
            }
        }
    }

    pub fn github(is_dark: bool) -> Self {
        if is_dark {
            Self {
                id: "github",
                name: "GitHub Modern",
                is_dark: true,
                bg: rgb(0x0d1117).into(),
                surface: rgb(0x161b22).into(),
                border: rgb(0x30363d).into(),
                text_primary: rgb(0xe6edf3).into(),
                text_muted: rgb(0x848d97).into(),
                accent: rgb(0x2f81f7).into(),
                code_bg: rgb(0x1f242c).into(),
                conflict_bg: rgb(0x3d1d20).into(),
                conflict_border: rgb(0xf85149).into(),
                syntax_keyword: rgb(0xff7b72).into(),
                syntax_string: rgb(0xa5d6ff).into(),
                syntax_comment: rgb(0x8b949e).into(),
                syntax_heading: rgb(0x79c0ff).into(),
                syntax_inline_code: rgb(0xc9d1d9).into(),
                syntax_link: rgb(0x58a6ff).into(),
            }
        } else {
            Self {
                id: "github",
                name: "GitHub Modern",
                is_dark: false,
                bg: rgb(0xf4f6f8).into(),
                surface: rgb(0xffffff).into(),
                border: rgb(0xd0d7de).into(),
                text_primary: rgb(0x1f2328).into(),
                text_muted: rgb(0x656d76).into(),
                accent: rgb(0x0969da).into(),
                code_bg: rgb(0xf1f5f9).into(),
                conflict_bg: rgb(0xffebe9).into(),
                conflict_border: rgb(0xcf222e).into(),
                syntax_keyword: rgb(0xcf222e).into(),
                syntax_string: rgb(0x0a3069).into(),
                syntax_comment: rgb(0x6e7781).into(),
                syntax_heading: rgb(0x0550ae).into(),
                syntax_inline_code: rgb(0x24292f).into(),
                syntax_link: rgb(0x0969da).into(),
            }
        }
    }

    pub fn solarized(is_dark: bool) -> Self {
        if is_dark {
            Self {
                id: "solarized",
                name: "Solarized",
                is_dark: true,
                bg: rgb(0x002b36).into(),
                surface: rgb(0x073642).into(),
                border: rgb(0x586e75).into(),
                text_primary: rgb(0x839496).into(),
                text_muted: rgb(0x657b83).into(),
                accent: rgb(0x2aa198).into(),
                code_bg: rgb(0x073642).into(),
                conflict_bg: rgb(0x3e1f24).into(),
                conflict_border: rgb(0xdc322f).into(),
                syntax_keyword: rgb(0x859900).into(),
                syntax_string: rgb(0x2aa198).into(),
                syntax_comment: rgb(0x586e75).into(),
                syntax_heading: rgb(0x268bd2).into(),
                syntax_inline_code: rgb(0x93a1a1).into(),
                syntax_link: rgb(0x6c71c4).into(),
            }
        } else {
            Self {
                id: "solarized",
                name: "Solarized",
                is_dark: false,
                bg: rgb(0xfdf6e3).into(),
                surface: rgb(0xeee8d5).into(),
                border: rgb(0x93a1a1).into(),
                text_primary: rgb(0x657b83).into(),
                text_muted: rgb(0x93a1a1).into(),
                accent: rgb(0x268bd2).into(),
                code_bg: rgb(0xeee8d5).into(),
                conflict_bg: rgb(0xfdf0ed).into(),
                conflict_border: rgb(0xdc322f).into(),
                syntax_keyword: rgb(0x859900).into(),
                syntax_string: rgb(0x2aa198).into(),
                syntax_comment: rgb(0x93a1a1).into(),
                syntax_heading: rgb(0x268bd2).into(),
                syntax_inline_code: rgb(0x073642).into(),
                syntax_link: rgb(0x6c71c4).into(),
            }
        }
    }

    pub fn one_dark(is_dark: bool) -> Self {
        if is_dark {
            Self {
                id: "one-dark",
                name: "One Dark Pro",
                is_dark: true,
                bg: rgb(0x21252b).into(),
                surface: rgb(0x282c34).into(),
                border: rgb(0x353b45).into(),
                text_primary: rgb(0xabb2bf).into(),
                text_muted: rgb(0x5c6370).into(),
                accent: rgb(0x61afef).into(),
                code_bg: rgb(0x1e2227).into(),
                conflict_bg: rgb(0x451a03).into(),
                conflict_border: rgb(0xb45309).into(),
                syntax_keyword: rgb(0xc678dd).into(),
                syntax_string: rgb(0x98c379).into(),
                syntax_comment: rgb(0x5c6370).into(),
                syntax_heading: rgb(0x61afef).into(),
                syntax_inline_code: rgb(0xabb2bf).into(),
                syntax_link: rgb(0x61afef).into(),
            }
        } else {
            Self {
                id: "one-dark",
                name: "One Dark Pro",
                is_dark: false,
                bg: rgb(0xfafafa).into(),
                surface: rgb(0xf0f0f0).into(),
                border: rgb(0xe0e0e0).into(),
                text_primary: rgb(0x383a42).into(),
                text_muted: rgb(0xa0a1a7).into(),
                accent: rgb(0x4078f2).into(),
                code_bg: rgb(0xf4f4f4).into(),
                conflict_bg: rgb(0xfef2f2).into(),
                conflict_border: rgb(0xfecaca).into(),
                syntax_keyword: rgb(0xa626a4).into(),
                syntax_string: rgb(0x50a14f).into(),
                syntax_comment: rgb(0xa0a1a7).into(),
                syntax_heading: rgb(0x4078f2).into(),
                syntax_inline_code: rgb(0x383a42).into(),
                syntax_link: rgb(0x4078f2).into(),
            }
        }
    }
}
