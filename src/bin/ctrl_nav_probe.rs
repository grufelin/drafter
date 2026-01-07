use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::Parser;

use drafter::keyboard::{
    keystroke_for_output_char, KeyStroke, KEY_LEFT, KEY_LEFTCTRL, KEY_LEFTSHIFT, KEY_RIGHT,
};
use drafter::keymap::us_qwerty_keymap;
use drafter::model::{Action, KeyState, Plan, PlanConfig};

#[derive(Debug, Parser)]
#[command(about = "Generate fast Ctrl+Left/Right probe plans", long_about = None)]
struct Args {
    /// Output path for the Ctrl+Left probe plan JSON.
    #[arg(long, default_value = "ctrl_left_probe.plan.json")]
    out_left: PathBuf,

    /// Output path for the Ctrl+Right probe plan JSON.
    #[arg(long, default_value = "ctrl_right_probe.plan.json")]
    out_right: PathBuf,

    /// Number of Ctrl+Arrow steps.
    #[arg(long, default_value_t = 35)]
    steps: u8,

    /// Press->release hold time for each key tap.
    #[arg(long, default_value_t = 3)]
    hold_ms: u64,

    /// Delay after each key release.
    #[arg(long, default_value_t = 1)]
    between_ms: u64,

    /// Delay around modifier transitions (Ctrl/Shift changes).
    #[arg(long, default_value_t = 1)]
    modifier_ms: u64,
}

#[derive(Debug, Clone)]
struct FastPlanBuilder {
    actions: Vec<Action>,
    shift_down: bool,
    ctrl_down: bool,
    shift_mask: u32,
    ctrl_mask: u32,
    hold_ms: u64,
    between_ms: u64,
    modifier_ms: u64,
}

impl FastPlanBuilder {
    fn new(
        shift_mask: u32,
        ctrl_mask: u32,
        hold_ms: u64,
        between_ms: u64,
        modifier_ms: u64,
    ) -> Self {
        let mut builder = Self {
            actions: Vec::new(),
            shift_down: false,
            ctrl_down: false,
            shift_mask,
            ctrl_mask,
            hold_ms,
            between_ms,
            modifier_ms,
        };

        builder.push_modifiers();
        builder
    }

    fn into_actions(self) -> Vec<Action> {
        self.actions
    }

    fn wait(&mut self, ms: u64) {
        if ms == 0 {
            return;
        }
        self.actions.push(Action::Wait { ms });
    }

    fn key(&mut self, keycode: u32, state: KeyState) {
        self.actions.push(Action::Key { keycode, state });
    }

    fn push_modifiers(&mut self) {
        let mut depressed = 0u32;
        if self.shift_down {
            depressed |= self.shift_mask;
        }
        if self.ctrl_down {
            depressed |= self.ctrl_mask;
        }

        self.actions.push(Action::Modifiers {
            mods_depressed: depressed,
            mods_latched: 0,
            mods_locked: 0,
            group: 0,
        });
    }

    fn set_shift(&mut self, down: bool) {
        if self.shift_down == down {
            return;
        }

        self.key(
            KEY_LEFTSHIFT,
            if down {
                KeyState::Pressed
            } else {
                KeyState::Released
            },
        );
        self.wait(self.modifier_ms);
        self.shift_down = down;
        self.push_modifiers();
        self.wait(self.modifier_ms);
    }

    fn set_ctrl(&mut self, down: bool) {
        if self.ctrl_down == down {
            return;
        }

        self.key(
            KEY_LEFTCTRL,
            if down {
                KeyState::Pressed
            } else {
                KeyState::Released
            },
        );
        self.wait(self.modifier_ms);
        self.ctrl_down = down;
        self.push_modifiers();
        self.wait(self.modifier_ms);
    }

    fn tap_key(&mut self, keycode: u32) {
        self.key(keycode, KeyState::Pressed);
        self.wait(self.hold_ms);
        self.key(keycode, KeyState::Released);
        self.wait(self.between_ms);
    }

    fn type_char(&mut self, stroke: KeyStroke) {
        self.set_ctrl(false);
        self.set_shift(stroke.shift);
        self.tap_key(stroke.keycode);
    }

    fn type_string(&mut self, s: &str) -> Result<()> {
        for c in s.chars() {
            let stroke =
                keystroke_for_output_char(c).ok_or_else(|| anyhow!("unsupported char {c:?}"))?;
            self.type_char(stroke);
        }
        Ok(())
    }

    fn nav_left(&mut self) {
        self.set_ctrl(false);
        self.set_shift(false);
        self.tap_key(KEY_LEFT);
    }

    fn ctrl_left(&mut self) {
        self.set_shift(false);
        self.set_ctrl(true);
        self.tap_key(KEY_LEFT);
        self.set_ctrl(false);
    }

    fn ctrl_right(&mut self) {
        self.set_shift(false);
        self.set_ctrl(true);
        self.tap_key(KEY_RIGHT);
        self.set_ctrl(false);
    }
}

fn write_plan(path: &PathBuf, plan: &Plan) -> Result<()> {
    let json = serde_json::to_string_pretty(plan).context("failed to serialize plan")?;
    fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))
}

fn build_ctrl_left_plan(args: &Args) -> Result<Plan> {
    let keymap = us_qwerty_keymap()?;
    let mut b = FastPlanBuilder::new(
        keymap.shift_mask,
        keymap.ctrl_mask,
        args.hold_ms,
        args.between_ms,
        args.modifier_ms,
    );

    let steps = args.steps;
    for idx in 1..=steps {
        let marker = format!("<{idx}>");

        b.ctrl_left();
        b.type_string(&marker)?;

        // Move back over the marker so the next Ctrl+Left starts
        // from the same landing position.
        for _ in 0..marker.chars().count() {
            b.nav_left();
        }
    }

    b.set_shift(false);
    b.set_ctrl(false);
    b.push_modifiers();

    Ok(Plan {
        version: 1,
        config: PlanConfig {
            layout: keymap.layout,
            keymap_format: keymap.keymap_format,
            keymap: keymap.keymap,
            wpm_target: 999.0,
        },
        actions: b.into_actions(),
    })
}

fn build_ctrl_right_plan(args: &Args) -> Result<Plan> {
    let keymap = us_qwerty_keymap()?;
    let mut b = FastPlanBuilder::new(
        keymap.shift_mask,
        keymap.ctrl_mask,
        args.hold_ms,
        args.between_ms,
        args.modifier_ms,
    );

    let steps = args.steps;
    for idx in 1..=steps {
        b.ctrl_right();
        b.type_string(&format!("<{idx}>"))?;
    }

    b.set_shift(false);
    b.set_ctrl(false);
    b.push_modifiers();

    Ok(Plan {
        version: 1,
        config: PlanConfig {
            layout: keymap.layout,
            keymap_format: keymap.keymap_format,
            keymap: keymap.keymap,
            wpm_target: 999.0,
        },
        actions: b.into_actions(),
    })
}

fn main() -> Result<()> {
    let args = Args::parse();

    let left = build_ctrl_left_plan(&args)?;
    let right = build_ctrl_right_plan(&args)?;

    write_plan(&args.out_left, &left)?;
    write_plan(&args.out_right, &right)?;

    eprintln!(
        "Wrote probe plans: {} and {}",
        args.out_left.display(),
        args.out_right.display()
    );

    Ok(())
}
