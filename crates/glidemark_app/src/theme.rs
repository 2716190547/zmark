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

impl ThemeColors {
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
                bg: rgb(0x18181a).into(),
                surface: rgb(0x222226).into(),
                border: rgb(0x2e2e33).into(),
                text_primary: rgb(0xf4f4f5).into(),
                text_muted: rgb(0x8a8a93).into(),
                accent: rgb(0xe26543).into(), // Warm terracotta matching logo
                code_bg: rgb(0x202024).into(),
                conflict_bg: rgb(0x382218).into(),
                conflict_border: rgb(0x5e3322).into(),
            }
        } else {
            Self {
                id: "default",
                name: "Default",
                is_dark: false,
                bg: rgb(0xffffff).into(),
                surface: rgb(0xfafaf9).into(),
                border: rgb(0xededed).into(), // Delicate hairline
                text_primary: rgb(0x1f2328).into(),
                text_muted: rgb(0x71717a).into(),
                accent: rgb(0xd9532f).into(), // Warm Terracotta Coral matching Logo
                code_bg: rgb(0xf4f4f5).into(),
                conflict_bg: rgb(0xfef2f2).into(),
                conflict_border: rgb(0xfecaca).into(),
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
            }
        } else {
            Self {
                id: "macos",
                name: "macOS Vibrant",
                is_dark: false,
                bg: rgb(0xffffff).into(),
                surface: rgb(0xf5f5f7).into(),
                border: rgb(0xe5e5ea).into(),
                text_primary: rgb(0x1d1d1f).into(),
                text_muted: rgb(0x86868b).into(),
                accent: rgb(0x0071e3).into(),
                code_bg: rgb(0xf0f0f2).into(),
                conflict_bg: rgb(0xfef3c7).into(),
                conflict_border: rgb(0xf59e0b).into(),
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
            }
        }
    }

    #[allow(dead_code)]
    pub fn catppuccin(is_dark: bool) -> Self {
        if is_dark {
            Self {
                id: "catppuccin",
                name: "Catppuccin",
                is_dark: true,
                bg: rgb(0x24273a).into(),
                surface: rgb(0x1e2030).into(),
                border: rgb(0x363a4f).into(),
                text_primary: rgb(0xcad3f5).into(),
                text_muted: rgb(0x8087a2).into(),
                accent: rgb(0xc6a0f6).into(),
                code_bg: rgb(0x181926).into(),
                conflict_bg: rgb(0x49202a).into(),
                conflict_border: rgb(0xed8796).into(),
            }
        } else {
            Self {
                id: "catppuccin",
                name: "Catppuccin",
                is_dark: false,
                bg: rgb(0xeff1f5).into(),
                surface: rgb(0xe6e9ef).into(),
                border: rgb(0xccd0da).into(),
                text_primary: rgb(0x4c4f69).into(),
                text_muted: rgb(0x7c7f93).into(),
                accent: rgb(0x8839ef).into(),
                code_bg: rgb(0xdce0e8).into(),
                conflict_bg: rgb(0xfce8ec).into(),
                conflict_border: rgb(0xe64553).into(),
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
            }
        }
    }

    #[allow(dead_code)]
    pub fn gruvbox(is_dark: bool) -> Self {
        if is_dark {
            Self {
                id: "gruvbox",
                name: "Gruvbox",
                is_dark: true,
                bg: rgb(0x282828).into(),
                surface: rgb(0x32302f).into(),
                border: rgb(0x504945).into(),
                text_primary: rgb(0xebdbb2).into(),
                text_muted: rgb(0xa89984).into(),
                accent: rgb(0xfabd2f).into(),
                code_bg: rgb(0x3c3836).into(),
                conflict_bg: rgb(0x502624).into(),
                conflict_border: rgb(0xfb4934).into(),
            }
        } else {
            Self {
                id: "gruvbox",
                name: "Gruvbox",
                is_dark: false,
                bg: rgb(0xfbf1c7).into(),
                surface: rgb(0xf2e5bc).into(),
                border: rgb(0xd5c4a1).into(),
                text_primary: rgb(0x282828).into(),
                text_muted: rgb(0x7c6f64).into(),
                accent: rgb(0xb57614).into(),
                code_bg: rgb(0xebdbb2).into(),
                conflict_bg: rgb(0xf9e0de).into(),
                conflict_border: rgb(0xcc241d).into(),
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
            }
        } else {
            Self {
                id: "github",
                name: "GitHub Modern",
                is_dark: false,
                bg: rgb(0xffffff).into(),
                surface: rgb(0xf6f8fa).into(),
                border: rgb(0xd0d7de).into(),
                text_primary: rgb(0x1f2328).into(),
                text_muted: rgb(0x656d76).into(),
                accent: rgb(0x0969da).into(),
                code_bg: rgb(0xf1f5f9).into(),
                conflict_bg: rgb(0xffebe9).into(),
                conflict_border: rgb(0xcf222e).into(),
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
            }
        }
    }
}
