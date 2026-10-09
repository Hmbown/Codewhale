//! Shared, data-only precise monetary display for diagnostic reports.
//! Currency selection and accounting remain authoritative on the host.

use crate::types::CommandCurrency;

#[must_use]
pub fn format_cost_amount_precise(amount: f64, currency: CommandCurrency) -> String {
    let symbol = match currency {
        CommandCurrency::Usd => "$",
        CommandCurrency::Cny => "¥",
    };
    if amount == 0.0 {
        format!("{symbol}0.0000")
    } else if amount > 0.0 && amount < 0.0001 {
        format!("<{symbol}0.0001")
    } else {
        format!("{symbol}{amount:.4}")
    }
}
