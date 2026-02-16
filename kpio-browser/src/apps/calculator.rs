//! Calculator
//!
//! Basic and scientific calculator.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use libm::{acos, asin, atan, ceil, cos, exp, fabs, floor, log, log10, pow, round, sin, sqrt, tan};

/// Calculator state
#[derive(Debug, Clone)]
pub struct Calculator {
    /// Display value
    pub display: String,
    /// Current value
    pub current: f64,
    /// Previous value
    pub previous: f64,
    /// Current operation
    pub operation: Option<Operation>,
    /// Waiting for operand
    pub waiting_for_operand: bool,
    /// Memory value
    pub memory: f64,
    /// History
    pub history: Vec<HistoryEntry>,
    /// Calculator mode
    pub mode: CalculatorMode,
    /// Angle mode for scientific
    pub angle_mode: AngleMode,
    /// Last result
    pub last_result: f64,
}

impl Calculator {
    /// Create new calculator
    pub fn new() -> Self {
        Self {
            display: String::from("0"),
            current: 0.0,
            previous: 0.0,
            operation: None,
            waiting_for_operand: true,
            memory: 0.0,
            history: Vec::new(),
            mode: CalculatorMode::Basic,
            angle_mode: AngleMode::Degrees,
            last_result: 0.0,
        }
    }

    /// Clear all
    pub fn clear(&mut self) {
        self.display = String::from("0");
        self.current = 0.0;
        self.previous = 0.0;
        self.operation = None;
        self.waiting_for_operand = true;
    }

    /// Clear entry
    pub fn clear_entry(&mut self) {
        self.display = String::from("0");
        self.current = 0.0;
        self.waiting_for_operand = true;
    }

    /// Input digit
    pub fn digit(&mut self, d: u8) {
        if self.waiting_for_operand {
            self.display = alloc::format!("{}", d);
            self.waiting_for_operand = false;
        } else if self.display.len() < 15 {
            if self.display == "0" {
                self.display = alloc::format!("{}", d);
            } else {
                self.display.push((b'0' + d) as char);
            }
        }
        self.current = self.display.parse().unwrap_or(0.0);
    }

    /// Input decimal point
    pub fn decimal(&mut self) {
        if self.waiting_for_operand {
            self.display = String::from("0.");
            self.waiting_for_operand = false;
        } else if !self.display.contains('.') {
            self.display.push('.');
        }
    }

    /// Backspace
    pub fn backspace(&mut self) {
        if !self.waiting_for_operand && self.display.len() > 1 {
            self.display.pop();
            self.current = self.display.parse().unwrap_or(0.0);
        } else {
            self.clear_entry();
        }
    }

    /// Negate
    pub fn negate(&mut self) {
        self.current = -self.current;
        self.display = format_number(self.current);
    }

    /// Percent
    pub fn percent(&mut self) {
        self.current /= 100.0;
        self.display = format_number(self.current);
    }

    /// Set operation
    pub fn set_operation(&mut self, op: Operation) {
        if !self.waiting_for_operand && self.operation.is_some() {
            self.calculate();
        }
        self.previous = self.current;
        self.operation = Some(op);
        self.waiting_for_operand = true;
    }

    /// Calculate result
    pub fn calculate(&mut self) {
        let result = match self.operation {
            Some(Operation::Add) => self.previous + self.current,
            Some(Operation::Subtract) => self.previous - self.current,
            Some(Operation::Multiply) => self.previous * self.current,
            Some(Operation::Divide) => {
                if self.current == 0.0 {
                    f64::NAN
                } else {
                    self.previous / self.current
                }
            }
            Some(Operation::Power) => pow(self.previous, self.current),
            Some(Operation::Root) => {
                if self.current == 0.0 {
                    f64::NAN
                } else {
                    pow(self.previous, 1.0 / self.current)
                }
            }
            Some(Operation::Mod) => self.previous % self.current,
            None => self.current,
        };

        // Add to history
        if self.operation.is_some() {
            self.history.push(HistoryEntry {
                expression: alloc::format!(
                    "{} {} {} =",
                    format_number(self.previous),
                    self.operation.as_ref().map(|o| o.symbol()).unwrap_or(""),
                    format_number(self.current)
                ),
                result,
            });
            if self.history.len() > 50 {
                self.history.remove(0);
            }
        }

        self.current = result;
        self.last_result = result;
        self.display = format_number(result);
        self.operation = None;
        self.waiting_for_operand = true;
    }

    /// Square root
    pub fn sqrt(&mut self) {
        self.current = sqrt(self.current);
        self.display = format_number(self.current);
    }

    /// Square
    pub fn square(&mut self) {
        self.current = self.current * self.current;
        self.display = format_number(self.current);
    }

    /// Reciprocal (1/x)
    pub fn reciprocal(&mut self) {
        if self.current != 0.0 {
            self.current = 1.0 / self.current;
            self.display = format_number(self.current);
        } else {
            self.display = String::from("Error");
        }
    }

    // Scientific functions

    /// Sine
    pub fn sin(&mut self) {
        let angle = self.to_radians(self.current);
        self.current = sin(angle);
        self.display = format_number(self.current);
    }

    /// Cosine
    pub fn cos(&mut self) {
        let angle = self.to_radians(self.current);
        self.current = cos(angle);
        self.display = format_number(self.current);
    }

    /// Tangent
    pub fn tan(&mut self) {
        let angle = self.to_radians(self.current);
        self.current = tan(angle);
        self.display = format_number(self.current);
    }

    /// Arc sine
    pub fn asin(&mut self) {
        if self.current >= -1.0 && self.current <= 1.0 {
            self.current = self.from_radians(asin(self.current));
            self.display = format_number(self.current);
        } else {
            self.display = String::from("Error");
        }
    }

    /// Arc cosine
    pub fn acos(&mut self) {
        if self.current >= -1.0 && self.current <= 1.0 {
            self.current = self.from_radians(acos(self.current));
            self.display = format_number(self.current);
        } else {
            self.display = String::from("Error");
        }
    }

    /// Arc tangent
    pub fn atan(&mut self) {
        self.current = self.from_radians(atan(self.current));
        self.display = format_number(self.current);
    }

    /// Natural log
    pub fn ln(&mut self) {
        if self.current > 0.0 {
            self.current = log(self.current);
            self.display = format_number(self.current);
        } else {
            self.display = String::from("Error");
        }
    }

    /// Log base 10
    pub fn log10(&mut self) {
        if self.current > 0.0 {
            self.current = log10(self.current);
            self.display = format_number(self.current);
        } else {
            self.display = String::from("Error");
        }
    }

    /// e^x
    pub fn exp(&mut self) {
        self.current = exp(self.current);
        self.display = format_number(self.current);
    }

    /// 10^x
    pub fn pow10(&mut self) {
        self.current = pow(10.0_f64, self.current);
        self.display = format_number(self.current);
    }

    /// Factorial
    pub fn factorial(&mut self) {
        let frac_part = self.current - floor(self.current);
        if self.current >= 0.0 && self.current <= 170.0 && frac_part == 0.0 {
            let n = self.current as u64;
            let mut result = 1.0_f64;
            for i in 2..=n {
                result *= i as f64;
            }
            self.current = result;
            self.display = format_number(self.current);
        } else {
            self.display = String::from("Error");
        }
    }

    /// Pi constant
    pub fn pi(&mut self) {
        self.current = core::f64::consts::PI;
        self.display = format_number(self.current);
        self.waiting_for_operand = false;
    }

    /// E constant
    pub fn euler(&mut self) {
        self.current = core::f64::consts::E;
        self.display = format_number(self.current);
        self.waiting_for_operand = false;
    }

    /// Absolute value
    pub fn abs(&mut self) {
        self.current = fabs(self.current);
        self.display = format_number(self.current);
    }

    /// Floor
    pub fn floor(&mut self) {
        self.current = floor(self.current);
        self.display = format_number(self.current);
    }

    /// Ceil
    pub fn ceil(&mut self) {
        self.current = ceil(self.current);
        self.display = format_number(self.current);
    }

    /// Round
    pub fn round(&mut self) {
        self.current = round(self.current);
        self.display = format_number(self.current);
    }

    // Memory functions

    /// Memory clear
    pub fn memory_clear(&mut self) {
        self.memory = 0.0;
    }

    /// Memory recall
    pub fn memory_recall(&mut self) {
        self.current = self.memory;
        self.display = format_number(self.current);
        self.waiting_for_operand = false;
    }

    /// Memory add
    pub fn memory_add(&mut self) {
        self.memory += self.current;
    }

    /// Memory subtract
    pub fn memory_subtract(&mut self) {
        self.memory -= self.current;
    }

    /// Memory store
    pub fn memory_store(&mut self) {
        self.memory = self.current;
    }

    // Helpers

    fn to_radians(&self, value: f64) -> f64 {
        match self.angle_mode {
            AngleMode::Radians => value,
            AngleMode::Degrees => value * core::f64::consts::PI / 180.0,
            AngleMode::Gradians => value * core::f64::consts::PI / 200.0,
        }
    }

    fn from_radians(&self, value: f64) -> f64 {
        match self.angle_mode {
            AngleMode::Radians => value,
            AngleMode::Degrees => value * 180.0 / core::f64::consts::PI,
            AngleMode::Gradians => value * 200.0 / core::f64::consts::PI,
        }
    }

    /// Toggle mode
    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            CalculatorMode::Basic => CalculatorMode::Scientific,
            CalculatorMode::Scientific => CalculatorMode::Programmer,
            CalculatorMode::Programmer => CalculatorMode::Basic,
        };
    }

    /// Clear history
    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}

impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Format number for display
fn format_number(n: f64) -> String {
    if n.is_nan() {
        return String::from("Error");
    }
    if n.is_infinite() {
        return if n.is_sign_positive() {
            String::from("∞")
        } else {
            String::from("-∞")
        };
    }

    let abs_n = fabs(n);
    let frac_part = n - floor(n);
    let s = if abs_n >= 1e15 || (n != 0.0 && abs_n < 1e-10) {
        alloc::format!("{:e}", n)
    } else if frac_part == 0.0 && abs_n < 1e15 {
        alloc::format!("{:.0}", n)
    } else {
        let s = alloc::format!("{:.10}", n);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };

    // Limit length
    if s.len() > 15 {
        s[..15].to_string()
    } else {
        s
    }
}

/// Operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Root,
    Mod,
}

impl Operation {
    /// Get symbol
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "−",
            Self::Multiply => "×",
            Self::Divide => "÷",
            Self::Power => "^",
            Self::Root => "√",
            Self::Mod => "mod",
        }
    }
}

/// Calculator mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CalculatorMode {
    #[default]
    Basic,
    Scientific,
    Programmer,
}

/// Angle mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AngleMode {
    #[default]
    Degrees,
    Radians,
    Gradians,
}

/// History entry
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// Expression
    pub expression: String,
    /// Result
    pub result: f64,
}

// =============================================================================
// Programmer Calculator
// =============================================================================

/// Number base
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumberBase {
    Binary,
    Octal,
    #[default]
    Decimal,
    Hexadecimal,
}

impl NumberBase {
    /// Get radix
    pub fn radix(&self) -> u32 {
        match self {
            Self::Binary => 2,
            Self::Octal => 8,
            Self::Decimal => 10,
            Self::Hexadecimal => 16,
        }
    }
}

/// Bit width
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BitWidth {
    Byte,  // 8-bit
    Word,  // 16-bit
    DWord, // 32-bit
    #[default]
    QWord, // 64-bit
}

impl BitWidth {
    /// Get bits
    pub fn bits(&self) -> u32 {
        match self {
            Self::Byte => 8,
            Self::Word => 16,
            Self::DWord => 32,
            Self::QWord => 64,
        }
    }

    /// Get max value (unsigned)
    pub fn max_value(&self) -> u64 {
        match self {
            Self::Byte => u8::MAX as u64,
            Self::Word => u16::MAX as u64,
            Self::DWord => u32::MAX as u64,
            Self::QWord => u64::MAX,
        }
    }
}

/// Programmer calculator state
#[derive(Debug, Clone)]
pub struct ProgrammerCalculator {
    /// Current value
    pub value: i64,
    /// Number base
    pub base: NumberBase,
    /// Bit width
    pub bit_width: BitWidth,
    /// Display hex uppercase
    pub uppercase: bool,
}

impl ProgrammerCalculator {
    /// Create new
    pub fn new() -> Self {
        Self {
            value: 0,
            base: NumberBase::Decimal,
            bit_width: BitWidth::QWord,
            uppercase: true,
        }
    }

    /// Get display string
    pub fn display(&self) -> String {
        match self.base {
            NumberBase::Binary => alloc::format!("{:b}", self.value),
            NumberBase::Octal => alloc::format!("{:o}", self.value),
            NumberBase::Decimal => alloc::format!("{}", self.value),
            NumberBase::Hexadecimal => {
                if self.uppercase {
                    alloc::format!("{:X}", self.value)
                } else {
                    alloc::format!("{:x}", self.value)
                }
            }
        }
    }

    /// Bitwise NOT
    pub fn not(&mut self) {
        self.value = !self.value;
        self.mask_to_width();
    }

    /// Bitwise AND
    pub fn and(&mut self, other: i64) {
        self.value &= other;
    }

    /// Bitwise OR
    pub fn or(&mut self, other: i64) {
        self.value |= other;
    }

    /// Bitwise XOR
    pub fn xor(&mut self, other: i64) {
        self.value ^= other;
    }

    /// Left shift
    pub fn left_shift(&mut self, bits: u32) {
        self.value <<= bits;
        self.mask_to_width();
    }

    /// Right shift
    pub fn right_shift(&mut self, bits: u32) {
        self.value >>= bits;
    }

    /// Mask to bit width
    fn mask_to_width(&mut self) {
        let mask = match self.bit_width {
            BitWidth::Byte => 0xFF,
            BitWidth::Word => 0xFFFF,
            BitWidth::DWord => 0xFFFFFFFF,
            BitWidth::QWord => -1i64, // All bits
        };
        self.value &= mask;
    }
}

impl Default for ProgrammerCalculator {
    fn default() -> Self {
        Self::new()
    }
}
