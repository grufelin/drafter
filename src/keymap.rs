use anyhow::{anyhow, Result};
use xkbcommon::xkb;

pub const KEYMAP_FORMAT_XKB_V1: u32 = 1;

#[derive(Debug, Clone)]
pub struct KeymapInfo {
    pub layout: String,
    pub keymap_format: u32,
    pub keymap: String,
    pub shift_mask: u32,
    pub ctrl_mask: u32,
}

pub fn us_qwerty_keymap() -> Result<KeymapInfo> {
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);

    let keymap = xkb::Keymap::new_from_names(
        &context,
        "evdev",
        "pc105",
        "us",
        "",
        None,
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .ok_or_else(|| anyhow!("failed to build xkb keymap for us/pc105"))?;

    let keymap_str = keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1);

    let shift_index = keymap.mod_get_index(xkb::MOD_NAME_SHIFT);
    if shift_index == xkb::MOD_INVALID {
        return Err(anyhow!("xkb keymap missing Shift modifier"));
    }

    let ctrl_index = keymap.mod_get_index(xkb::MOD_NAME_CTRL);
    if ctrl_index == xkb::MOD_INVALID {
        return Err(anyhow!("xkb keymap missing Control modifier"));
    }

    let shift_mask = 1u32
        .checked_shl(shift_index)
        .ok_or_else(|| anyhow!("Shift modifier index out of range"))?;
    let ctrl_mask = 1u32
        .checked_shl(ctrl_index)
        .ok_or_else(|| anyhow!("Control modifier index out of range"))?;

    Ok(KeymapInfo {
        layout: "us".to_string(),
        keymap_format: KEYMAP_FORMAT_XKB_V1,
        keymap: keymap_str,
        shift_mask,
        ctrl_mask,
    })
}
